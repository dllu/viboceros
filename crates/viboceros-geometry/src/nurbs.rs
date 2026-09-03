use std::ops::RangeInclusive;

use crate::{
    AffineTransform3, BoundingBox3, Frame3, GeometryError, Point3, Polyline3, Real, Tolerance,
    Vector3, integration::integrate_adaptive, require_finite,
};

// OpenNURBS uses these fixed coordinate tolerances for Curve::IsClosed.
pub(crate) const CURVE_COINCIDENCE_ABSOLUTE: Real = 2.328_306_436_538_696_3e-10;
const CURVE_COINCIDENCE_RELATIVE: Real = 2.273_736_754_432_320_6e-13;
const OPENNURBS_SQRT_EPSILON: Real = 1.490_116_119_385e-8;

/// Topology requested for a Rhino `Curve`-style control-point curve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlPointCurveClosure {
    /// Leaves the natural endpoints independent.
    Open,
    /// Repeats controls and uniform knots to form a smooth periodic seam.
    Smooth,
    /// Repeats only the first control to form a non-periodic kinked seam.
    Sharp,
}

/// A Euclidean control point paired with a strictly positive rational weight.
///
/// Positive weights make every evaluated point a convex combination of its
/// active control points and guarantee a nonzero rational denominator in exact
/// arithmetic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedPoint3 {
    point: Point3,
    weight: Real,
}

impl WeightedPoint3 {
    pub fn try_new(point: Point3, weight: Real) -> Result<Self, GeometryError> {
        if weight.is_finite() && weight > 0.0 {
            Ok(Self { point, weight })
        } else {
            Err(GeometryError::InvalidWeight { index: 0 })
        }
    }

    #[inline]
    pub const fn point(self) -> Point3 {
        self.point
    }

    #[inline]
    pub const fn weight(self) -> Real {
        self.weight
    }
}

/// A finite, non-uniform rational B-spline curve using a standard full knot
/// vector of length `control_point_count + degree + 1`.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsCurve {
    degree: usize,
    control_points: Vec<WeightedPoint3>,
    knots: Vec<Real>,
    rational: bool,
}

impl NurbsCurve {
    /// Constructs a non-rational NURBS curve (all weights are one).
    pub fn try_new(
        degree: usize,
        control_points: Vec<Point3>,
        knots: Vec<Real>,
    ) -> Result<Self, GeometryError> {
        let control_points = control_points
            .into_iter()
            .map(|point| WeightedPoint3 { point, weight: 1.0 })
            .collect();
        Self::try_new_rational(degree, control_points, knots)
    }

    /// Constructs a rational NURBS curve after validating its full knot vector
    /// and all control-point weights.
    pub fn try_new_rational(
        degree: usize,
        control_points: Vec<WeightedPoint3>,
        knots: Vec<Real>,
    ) -> Result<Self, GeometryError> {
        validate_structure(degree, &control_points, &knots)?;
        let first_weight = control_points[0].weight;
        let rational = control_points
            .iter()
            .any(|control_point| control_point.weight != first_weight);
        Ok(Self {
            degree,
            control_points,
            knots,
            rational,
        })
    }

    /// Constructs Rhino's exact single-span conic from its endpoints, tangent
    /// intersection (apex), and rho value.
    ///
    /// `rho` is strictly between zero and one. The equivalent rational
    /// quadratic middle weight is `rho / (1 - rho)`; consequently values
    /// below, equal to, and above one half produce elliptic, parabolic, and
    /// hyperbolic segments respectively. The curve runs from `start` to `end`
    /// on the normalized `[0, 1]` domain. As Rhino does for typed rho input,
    /// coincident or collinear control points are retained rather than
    /// rejected.
    pub fn try_conic(
        start: Point3,
        apex: Point3,
        end: Point3,
        rho: Real,
    ) -> Result<Self, GeometryError> {
        require_finite([rho], "conic rho")?;
        if !(0.0..1.0).contains(&rho) {
            return Err(GeometryError::Degenerate { context: "conic" });
        }
        let middle_weight = rho / (1.0 - rho);
        Self::try_conic_with_middle_weight(start, apex, end, middle_weight)
    }

    /// Constructs Rhino's exact single-span conic through an interior point.
    ///
    /// Rhino projects the picked point orthogonally into the plane of the
    /// start, apex, and end points. Its barycentric coordinates in that
    /// control triangle determine the unique positive middle weight.
    pub fn try_conic_through_point(
        start: Point3,
        apex: Point3,
        end: Point3,
        through: Point3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let chord = start.vector_to(end)?;
        let to_apex = start.vector_to(apex)?;
        let to_through = start.vector_to(through)?;
        let chord_length = chord.length()?;
        if chord_length <= tolerance.absolute() {
            let apex_length = to_apex.length()?;
            let apex_direction = to_apex.normalized(tolerance)?;
            let rho = to_through.dot(apex_direction.as_vector())? / apex_length;
            if !(0.0..1.0).contains(&rho) {
                return Err(GeometryError::Degenerate { context: "conic" });
            }
            return Self::try_conic_with_middle_weight(start, apex, end, rho / (1.0 - rho));
        }

        let frame = Frame3::try_from_points(start, end, apex, tolerance)?;
        let apex_x = to_apex.dot(frame.x_axis().as_vector())?;
        let apex_y = to_apex.dot(frame.y_axis().as_vector())?;
        let through_x = to_through.dot(frame.x_axis().as_vector())?;
        let through_y = to_through.dot(frame.y_axis().as_vector())?;

        let apex_coefficient = through_y / apex_y;
        let end_coefficient = (-apex_coefficient).mul_add(apex_x, through_x) / chord_length;
        let start_coefficient = 1.0 - apex_coefficient - end_coefficient;
        require_finite(
            [apex_coefficient, end_coefficient, start_coefficient],
            "conic through-point coordinates",
        )?;
        if apex_coefficient <= 0.0 || end_coefficient <= 0.0 || start_coefficient <= 0.0 {
            return Err(GeometryError::Degenerate { context: "conic" });
        }

        // For rational Bernstein coefficients alpha, beta, gamma,
        // beta / (2 sqrt(alpha gamma)) is the middle control weight. Taking
        // the roots separately avoids underflow in alpha * gamma.
        let middle_weight =
            (0.5 * apex_coefficient / start_coefficient.sqrt()) / end_coefficient.sqrt();
        Self::try_conic_with_middle_weight(start, apex, end, middle_weight)
    }

    fn try_conic_with_middle_weight(
        start: Point3,
        apex: Point3,
        end: Point3,
        middle_weight: Real,
    ) -> Result<Self, GeometryError> {
        if !middle_weight.is_finite() || middle_weight <= 0.0 {
            return Err(GeometryError::Degenerate { context: "conic" });
        }
        Self::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(start, 1.0)?,
                WeightedPoint3::try_new(apex, middle_weight)?,
                WeightedPoint3::try_new(end, 1.0)?,
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
    }

    /// Constructs Rhino's exact quadratic parabola NURBS curve.
    ///
    /// The frame origin is the vertex, frame Z is the opening direction, and
    /// frame X points toward the picked positive endpoint. The full form runs
    /// from the mirrored negative endpoint to the positive endpoint; `half`
    /// runs from the vertex to the positive endpoint. Both use Rhino's
    /// normalized `[0, 1]` parameter domain.
    pub fn try_parabola(
        vertex_frame: Frame3,
        radius: Real,
        height: Real,
        half: bool,
    ) -> Result<Self, GeometryError> {
        require_finite([radius, height], "parabola dimensions")?;
        if radius <= 0.0 || height <= 0.0 {
            return Err(GeometryError::Degenerate {
                context: "parabola",
            });
        }
        let vertex = vertex_frame.origin();
        let point = |radial: Real, axial: Real| {
            vertex
                .translated(vertex_frame.x_axis().as_vector().scaled(radial)?)?
                .translated(vertex_frame.z_axis().as_vector().scaled(axial)?)
        };
        let controls = if half {
            vec![vertex, point(0.5 * radius, 0.0)?, point(radius, height)?]
        } else {
            vec![
                point(-radius, height)?,
                point(0.0, -height)?,
                point(radius, height)?,
            ]
        };
        Self::try_new(2, controls, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0])
    }

    /// Constructs Rhino's quadratic parabola segment from its vertex and two
    /// endpoints. The vertex may lie outside the returned segment; the curve
    /// direction always runs from `start` to `end`.
    pub fn try_parabola_from_vertex(
        vertex: Point3,
        start: Point3,
        end: Point3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let vertex_to_start = vertex.vector_to(start)?;
        let vertex_to_end = vertex.vector_to(end)?;
        let start_length = vertex_to_start.length()?;
        let end_length = vertex_to_end.length()?;
        if start_length <= tolerance.absolute() || end_length <= tolerance.absolute() {
            return Err(GeometryError::Degenerate {
                context: "three-point parabola",
            });
        }

        // Scaling both vectors by the same value leaves the vertex equation
        // unchanged and avoids overflowing its squared coefficients.
        let scale = start_length.max(end_length);
        let scaled_start = vector_divided_by(vertex_to_start, scale)?;
        let scaled_end = vector_divided_by(vertex_to_end, scale)?;
        let start_squared = scaled_start.dot(scaled_start)?;
        let mixed = scaled_start.dot(scaled_end)?;
        let end_squared = scaled_end.dot(scaled_end)?;
        if start_squared == 0.0 || end_squared == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "three-point parabola",
            });
        }

        // For q = P0 - 2 P1 + P2, imposing P(t) = vertex and
        // P'(t) dot q = 0 gives this scalar equation. Its endpoint signs
        // bracket the parameter belonging to the supplied vertex.
        let equation = |parameter: Real| {
            let complement = 1.0 - parameter;
            let left = -start_squared * complement * complement * complement;
            let middle = mixed * (2.0 * parameter - 1.0) * parameter * complement;
            let right = end_squared * parameter * parameter * parameter;
            left + middle + right
        };
        let mut lower = 0.0;
        let mut upper = 1.0;
        let mut parameter = 0.5;
        for _ in 0..128 {
            parameter = 0.5 * lower + 0.5 * upper;
            let value = equation(parameter);
            if value == 0.0 || parameter == lower || parameter == upper {
                break;
            }
            if value < 0.0 {
                lower = parameter;
            } else {
                upper = parameter;
            }
        }
        if !(0.0..1.0).contains(&parameter) {
            return Err(GeometryError::Degenerate {
                context: "three-point parabola",
            });
        }

        let complement = 1.0 - parameter;
        let quadratic = Vector3::try_new(
            vertex_to_start.x() / parameter + vertex_to_end.x() / complement,
            vertex_to_start.y() / parameter + vertex_to_end.y() / complement,
            vertex_to_start.z() / parameter + vertex_to_end.z() / complement,
        )?;
        quadratic_parabola_from_second_difference(start, end, quadratic, tolerance)
    }

    /// Constructs Rhino's quadratic parabola segment from a focus and two
    /// endpoints. Of the two possible axes, this matches Rhino by selecting
    /// the valid solution with the smaller positive focal distance.
    pub fn try_parabola_from_focus(
        focus: Point3,
        start: Point3,
        end: Point3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let focus_to_start = focus.vector_to(start)?;
        let focus_to_end = focus.vector_to(end)?;
        let chord = start.vector_to(end)?;
        let start_distance = focus_to_start.length()?;
        let end_distance = focus_to_end.length()?;
        let chord_length = chord.length()?;
        if start_distance <= tolerance.absolute()
            || end_distance <= tolerance.absolute()
            || chord_length <= tolerance.absolute()
        {
            return Err(GeometryError::Degenerate {
                context: "three-point parabola",
            });
        }

        let chord_direction = chord.normalized(tolerance)?;
        let start_along_chord = focus_to_start.dot(chord_direction.as_vector())?;
        let normal_component = subtract_scaled_vector(
            focus_to_start,
            chord_direction.as_vector(),
            start_along_chord,
        )?;
        let normal_direction = normal_component.normalized(tolerance)?;
        let alpha = ((end_distance - start_distance) / chord_length).clamp(-1.0, 1.0);
        let beta = ((1.0 - alpha).max(0.0) * (1.0 + alpha).max(0.0)).sqrt();

        let mut selected: Option<(Vector3, Real)> = None;
        for side in [-1.0, 1.0] {
            let axis = Vector3::try_new(
                alpha.mul_add(chord_direction.x(), side * beta * normal_direction.x()),
                alpha.mul_add(chord_direction.y(), side * beta * normal_direction.y()),
                alpha.mul_add(chord_direction.z(), side * beta * normal_direction.z()),
            )?
            .normalized_nonzero()?
            .as_vector();
            let axial = focus_to_start.dot(axis)?;
            let radial = subtract_scaled_vector(focus_to_start, axis, axial)?;
            let radial_length = radial.length()?;
            let focal_distance = if axial >= 0.0 {
                let denominator = start_distance + axial;
                if denominator <= 0.0 {
                    0.0
                } else {
                    0.5 * radial_length * (radial_length / denominator)
                }
            } else {
                0.5 * (start_distance - axial)
            };
            if focal_distance.is_finite()
                && focal_distance > tolerance.absolute()
                && selected.is_none_or(|(_, best)| focal_distance < best)
            {
                selected = Some((axis, focal_distance));
            }
        }
        let Some((axis, focal_distance)) = selected else {
            return Err(GeometryError::Degenerate {
                context: "three-point parabola",
            });
        };

        let chord_axial = chord.dot(axis)?;
        let radial_chord = subtract_scaled_vector(chord, axis, chord_axial)?;
        let radial_length = radial_chord.length()?;
        if radial_length <= tolerance.absolute() {
            return Err(GeometryError::Degenerate {
                context: "three-point parabola",
            });
        }
        let quadratic_length = 0.25 * radial_length * (radial_length / focal_distance);
        let quadratic = axis.scaled(quadratic_length)?;
        quadratic_parabola_from_second_difference(start, end, quadratic, tolerance)
    }

    /// Constructs a quadratic parabola through an interior point with the
    /// supplied opening direction. `through` must project strictly between
    /// the projected endpoints along that direction.
    pub fn try_parabola_through_point(
        start: Point3,
        through: Point3,
        end: Point3,
        opening_direction: Vector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let axis = opening_direction.normalized(tolerance)?.as_vector();
        let chord = start.vector_to(end)?;
        let to_through = start.vector_to(through)?;
        let chord_axial = chord.dot(axis)?;
        let through_axial = to_through.dot(axis)?;
        let projected_chord = subtract_scaled_vector(chord, axis, chord_axial)?;
        let projected_through = subtract_scaled_vector(to_through, axis, through_axial)?;
        let projected_length = projected_chord.length()?;
        if projected_length <= tolerance.absolute() {
            return Err(GeometryError::Degenerate {
                context: "three-point parabola",
            });
        }
        let projected_direction = projected_chord.normalized_nonzero()?;
        let parameter = projected_through.dot(projected_direction.as_vector())? / projected_length;
        if !(0.0..1.0).contains(&parameter) {
            return Err(GeometryError::Degenerate {
                context: "three-point parabola",
            });
        }

        let projected_residual =
            subtract_scaled_vector(projected_through, projected_chord, parameter)?;
        let residual_length = projected_residual.length()?;
        let input_scale = projected_length.max(projected_through.length()?);
        let residual_limit = tolerance.absolute().max(tolerance.relative() * input_scale);
        if residual_length > residual_limit {
            return Err(GeometryError::Degenerate {
                context: "three-point parabola",
            });
        }

        let axial_residual = (-parameter).mul_add(chord_axial, through_axial);
        let denominator = parameter * (1.0 - parameter);
        let quadratic_length = -axial_residual / denominator;
        if !quadratic_length.is_finite() || quadratic_length <= tolerance.absolute() {
            return Err(GeometryError::Degenerate {
                context: "three-point parabola",
            });
        }
        let quadratic = axis.scaled(quadratic_length)?;
        quadratic_parabola_from_second_difference(start, end, quadratic, tolerance)
    }

    /// Returns the focus of a non-degenerate, single-span, non-rational
    /// quadratic parabola.
    pub fn try_parabola_focus(&self, tolerance: Tolerance) -> Result<Point3, GeometryError> {
        if self.degree != 2 || self.control_points.len() != 3 || self.rational {
            return Err(GeometryError::Degenerate {
                context: "quadratic parabola",
            });
        }
        let first = self.control_points[0].point;
        let middle = self.control_points[1].point;
        let last = self.control_points[2].point;
        let middle_to_first = middle.vector_to(first)?;
        let middle_to_last = middle.vector_to(last)?;
        let quadratic = add_vectors(middle_to_first, middle_to_last)?;
        let quadratic_length = quadratic.length()?;
        let axis = quadratic.normalized(tolerance)?.as_vector();
        let linear = first.vector_to(middle)?.scaled(2.0)?;
        let axial_linear = linear.dot(axis)?;
        let tangent = subtract_scaled_vector(linear, axis, axial_linear)?;
        let tangent_length = tangent.length()?;
        if tangent_length <= tolerance.absolute() {
            return Err(GeometryError::Degenerate {
                context: "quadratic parabola",
            });
        }
        let focal_distance = 0.25 * tangent_length * (tangent_length / quadratic_length);
        if !focal_distance.is_finite() || focal_distance <= tolerance.absolute() {
            return Err(GeometryError::Degenerate {
                context: "quadratic parabola",
            });
        }
        let vertex_parameter = -0.5 * axial_linear / quadratic_length;
        let vertex = quadratic_power_point(first, linear, quadratic, vertex_parameter)?;
        vertex.translated(axis.scaled(focal_distance)?)
    }

    /// Constructs one exact branch segment of a centered hyperbola.
    ///
    /// The frame X axis points toward the branch, the frame Y axis points
    /// toward the positive endpoint, and `axial_extent` is that endpoint's
    /// positive X coordinate. The semi-axis coefficients satisfy
    /// `x^2 / a^2 - y^2 / b^2 = 1`. Rhino represents this symmetric segment
    /// as one rational quadratic span on the normalized `[0, 1]` domain.
    pub fn try_hyperbola(
        center_frame: Frame3,
        semi_transverse_axis: Real,
        semi_conjugate_axis: Real,
        axial_extent: Real,
    ) -> Result<Self, GeometryError> {
        require_finite(
            [semi_transverse_axis, semi_conjugate_axis, axial_extent],
            "hyperbola dimensions",
        )?;
        if semi_transverse_axis <= 0.0
            || semi_conjugate_axis <= 0.0
            || axial_extent <= semi_transverse_axis
        {
            return Err(GeometryError::Degenerate {
                context: "hyperbola",
            });
        }

        let middle_weight = axial_extent / semi_transverse_axis;
        if !middle_weight.is_finite() || middle_weight <= 1.0 {
            return Err(GeometryError::Degenerate {
                context: "hyperbola",
            });
        }
        let hyperbolic_sine = (middle_weight - 1.0).sqrt() * (middle_weight + 1.0).sqrt();
        let radial_extent = semi_conjugate_axis * hyperbolic_sine;
        let middle_axial = semi_transverse_axis / middle_weight;
        require_finite(
            [middle_weight, radial_extent, middle_axial],
            "hyperbola controls",
        )?;
        if radial_extent <= 0.0 {
            return Err(GeometryError::Degenerate {
                context: "hyperbola",
            });
        }

        let point = |axial: Real, radial: Real| {
            center_frame
                .origin()
                .translated(center_frame.x_axis().as_vector().scaled(axial)?)?
                .translated(center_frame.y_axis().as_vector().scaled(radial)?)
        };
        Self::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(axial_extent, -radial_extent)?, 1.0)?,
                WeightedPoint3::try_new(point(middle_axial, 0.0)?, middle_weight)?,
                WeightedPoint3::try_new(point(axial_extent, radial_extent)?, 1.0)?,
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
    }

    /// Constructs an open, clamped, uniformly spaced non-rational curve.
    pub fn try_clamped_uniform(
        degree: usize,
        control_points: Vec<Point3>,
    ) -> Result<Self, GeometryError> {
        let knots = clamped_uniform_knots(degree, control_points.len())?;
        Self::try_new(degree, control_points, knots)
    }

    /// Constructs a Rhino `Curve`-style open control-point curve.
    ///
    /// The effective degree is lowered to `control_point_count - 1` when the
    /// requested degree is too high. Knots are clamped and uniform, and the
    /// parameter domain is scaled to the control polygon's total length.
    pub fn try_control_point_curve(
        requested_degree: usize,
        control_points: Vec<Point3>,
    ) -> Result<Self, GeometryError> {
        Self::try_control_point_curve_with_closure(
            requested_degree,
            control_points,
            ControlPointCurveClosure::Open,
        )
    }

    /// Constructs a Rhino `Curve`-style control-point curve with the requested
    /// seam topology.
    pub fn try_control_point_curve_with_closure(
        requested_degree: usize,
        mut control_points: Vec<Point3>,
        closure: ControlPointCurveClosure,
    ) -> Result<Self, GeometryError> {
        if requested_degree == 0 {
            return Err(GeometryError::InvalidDegree);
        }

        if closure != ControlPointCurveClosure::Open
            && control_points.len() > 1
            && control_points.first() == control_points.last()
        {
            control_points.pop();
        }

        match closure {
            ControlPointCurveClosure::Open => {
                if control_points.len() < 2 {
                    return Err(GeometryError::InsufficientControlPoints {
                        degree: 1,
                        required: 2,
                        actual: control_points.len(),
                    });
                }
                let degree = requested_degree.min(control_points.len() - 1);
                clamped_control_point_curve(degree, control_points)
            }
            ControlPointCurveClosure::Smooth | ControlPointCurveClosure::Sharp => {
                if control_points.len() < 3 {
                    return Err(GeometryError::InsufficientClosedControlPoints {
                        actual: control_points.len(),
                    });
                }
                if closure == ControlPointCurveClosure::Smooth {
                    let degree = requested_degree.min(control_points.len());
                    periodic_control_point_curve(degree, control_points)
                } else {
                    control_points.push(control_points[0]);
                    let degree = requested_degree.min(control_points.len() - 1);
                    clamped_control_point_curve(degree, control_points)
                }
            }
        }
    }

    #[inline]
    pub const fn degree(&self) -> usize {
        self.degree
    }

    #[inline]
    pub fn control_points(&self) -> &[WeightedPoint3] {
        &self.control_points
    }

    #[inline]
    pub fn knots(&self) -> &[Real] {
        &self.knots
    }

    #[inline]
    pub const fn is_rational(&self) -> bool {
        self.rational
    }

    pub fn domain(&self) -> RangeInclusive<Real> {
        self.knots[self.degree]..=self.knots[self.control_points.len()]
    }

    /// Affinely maps the full knot vector onto a new active parameter domain.
    ///
    /// Control points and weights are unchanged, so the geometric image and
    /// normalized parameter direction are preserved. Knots outside the active
    /// domain, as used by periodic curves, are extrapolated by the same map.
    pub fn try_reparameterized(&self, domain: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        let target_start = *domain.start();
        let target_end = *domain.end();
        if !target_start.is_finite() || !target_end.is_finite() {
            return Err(GeometryError::InvalidKnotVector {
                context: "the reparameterized domain must be finite",
            });
        }
        if target_start >= target_end {
            return Err(GeometryError::InvalidKnotVector {
                context: "the reparameterized domain must have positive length",
            });
        }

        let source = self.domain();
        let source_start = *source.start();
        let source_end = *source.end();
        let knots = self
            .knots
            .iter()
            .map(|knot| {
                if *knot == source_start {
                    Ok(target_start)
                } else if *knot == source_end {
                    Ok(target_end)
                } else {
                    reparameterize_value(*knot, source_start, source_end, target_start, target_end)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_new_rational(self.degree, self.control_points.clone(), knots)
    }

    /// Returns every nonempty knot span in the active curve domain.
    pub fn spans(&self) -> impl Iterator<Item = (Real, Real)> + '_ {
        self.knots
            .windows(2)
            .skip(self.degree)
            .take(self.control_points.len() - self.degree)
            .filter_map(|knots| (knots[0] < knots[1]).then_some((knots[0], knots[1])))
    }

    /// Returns the OpenNURBS-compatible topological closed state.
    ///
    /// NURBS curves need at least four controls, coincident natural endpoints,
    /// and interior samples distinct from both ends. Endpoint coincidence uses
    /// OpenNURBS' fixed zero/relative tolerances rather than document tolerance.
    pub fn is_closed(&self) -> Result<bool, GeometryError> {
        if self.control_points.len() < 4 {
            return Ok(false);
        }
        if self.is_periodic() {
            return Ok(true);
        }
        let start = self.evaluate(*self.domain().start())?;
        let end = self.evaluate(*self.domain().end())?;
        if !curve_points_coincident(start, end) {
            return Ok(false);
        }
        let first_interior = self.evaluate(self.parameter_at(1.0 / 3.0)?)?;
        let second_interior = self.evaluate(self.parameter_at(2.0 / 3.0)?)?;
        Ok(!curve_points_coincident(start, first_interior)
            && !curve_points_coincident(start, second_interior)
            && !curve_points_coincident(end, first_interior)
            && !curve_points_coincident(end, second_interior))
    }

    /// Returns whether knots and repeated end controls form an OpenNURBS-style
    /// periodic curve. By convention, degree-one curves are never periodic.
    pub fn is_periodic(&self) -> bool {
        let order = self.degree + 1;
        let control_count = self.control_points.len();
        let short_knots = &self.knots[1..self.knots.len() - 1];
        if !knot_vector_is_periodic(order, control_count, short_knots) {
            return false;
        }
        (0..self.degree).all(|index| {
            let left = self.control_points[index].point;
            let right = self.control_points[control_count - self.degree + index].point;
            curve_points_coincident(left, right)
        })
    }

    /// Returns control-point locations in Rhino `ExtractPt` grip order.
    /// Periodic curves omit their repeated tail controls. Non-periodic closed
    /// curves omit an exactly duplicated final seam control, while
    /// near-coincident controls remain distinct.
    pub fn extract_point_locations(&self) -> Result<Vec<Point3>, GeometryError> {
        let mut points = self
            .control_points
            .iter()
            .map(|control_point| control_point.point)
            .collect::<Vec<_>>();
        if self.is_periodic() {
            points.truncate(points.len() - self.degree);
        } else if self.is_closed()? && points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        Ok(points)
    }

    /// Fits Rhino's extracted degree-one control polygon through this curve's
    /// Euclidean control locations.
    ///
    /// Periodic curves use the domain-aligned Greville window returned by
    /// `Curve.ControlPolygon`, retaining one repeated endpoint to close the
    /// polyline. Unlike `ExtractPt`, this can therefore rotate the first
    /// output point away from raw control index zero.
    pub fn control_polygon(&self, tolerance: Tolerance) -> Result<Polyline3, GeometryError> {
        let (start, end) = control_polygon_range(
            self.degree,
            self.control_points.len(),
            &self.knots,
            self.is_periodic(),
        );
        let mut points = self.control_points[start..end]
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>();
        if self.is_periodic() {
            let first = points[0];
            *points
                .last_mut()
                .expect("a periodic control window is nonempty") = first;
        }
        Polyline3::try_new(points, tolerance)
    }

    pub fn control_point_bounds(&self) -> BoundingBox3 {
        BoundingBox3::from_points(
            self.control_points
                .iter()
                .map(|control_point| control_point.point),
        )
        .expect("a valid NURBS curve has control points")
    }

    /// Maps a normalized value in `[0, 1]` into the curve domain without
    /// subtracting potentially opposite, very large endpoints.
    pub fn parameter_at(&self, normalized: Real) -> Result<Real, GeometryError> {
        if !normalized.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "normalized NURBS parameter",
            });
        }
        if !(0.0..=1.0).contains(&normalized) {
            return Err(GeometryError::ParameterOutOfDomain {
                parameter: normalized,
                domain_start: 0.0,
                domain_end: 1.0,
            });
        }
        let domain = self.domain();
        let start = *domain.start();
        let end = *domain.end();
        let parameter = start.mul_add(1.0 - normalized, end * normalized);
        require_finite([parameter], "NURBS parameter")?;
        Ok(parameter)
    }

    /// Evaluates the curve with the homogeneous de Boor algorithm.
    pub fn evaluate(&self, parameter: Real) -> Result<Point3, GeometryError> {
        let span = self.checked_span(parameter)?;
        let work = self.active_homogeneous_control_points(span)?;
        let homogeneous = de_boor(&self.knots, self.degree, span, parameter, work)?;
        project_homogeneous(homogeneous)
    }

    /// Evaluates the point and exact first derivative using the derivative
    /// control polygon in homogeneous coordinates and the rational quotient
    /// rule.
    pub fn evaluate_with_derivative(
        &self,
        parameter: Real,
    ) -> Result<(Point3, Vector3), GeometryError> {
        let span = self.checked_span(parameter)?;
        let active = self.active_homogeneous_control_points(span)?;
        let homogeneous = de_boor(&self.knots, self.degree, span, parameter, active.clone())?;
        let point = project_homogeneous(homogeneous)?;

        let first_control_point = span - self.degree;
        let mut derivative_controls = Vec::with_capacity(self.degree);
        for local_index in 0..self.degree {
            let control_point_index = first_control_point + local_index;
            let knot_start = self.knots[control_point_index + 1];
            let knot_end = self.knots[control_point_index + self.degree + 1];
            let mut derivative = [0.0; 4];
            for coordinate in 0..4 {
                derivative[coordinate] = stable_divided_difference(
                    active[local_index + 1][coordinate],
                    active[local_index][coordinate],
                    self.degree,
                    knot_start,
                    knot_end,
                )?;
            }
            derivative_controls.push(derivative);
        }

        let homogeneous_derivative = de_boor(
            &self.knots[1..self.knots.len() - 1],
            self.degree - 1,
            span - 1,
            parameter,
            derivative_controls,
        )?;
        let weight = homogeneous[3];
        let weight_derivative = homogeneous_derivative[3];
        let point_coordinates = point.to_array();
        let derivative: [Real; 3] = std::array::from_fn(|coordinate| {
            (-point_coordinates[coordinate])
                .mul_add(weight_derivative, homogeneous_derivative[coordinate])
                / weight
        });
        Ok((point, Vector3::try_from(derivative)?))
    }

    /// Evaluates the point and exact first and second derivatives using
    /// homogeneous derivative control polygons and the rational quotient
    /// rule.
    pub fn evaluate_with_second_derivative(
        &self,
        parameter: Real,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        let span = self.checked_span(parameter)?;
        let active = self.active_homogeneous_control_points(span)?;
        let homogeneous = de_boor(&self.knots, self.degree, span, parameter, active.clone())?;
        let point = project_homogeneous(homogeneous)?;

        let first_control_point = span - self.degree;
        let mut derivative_controls = Vec::with_capacity(self.degree);
        for local_index in 0..self.degree {
            let control_point_index = first_control_point + local_index;
            let knot_start = self.knots[control_point_index + 1];
            let knot_end = self.knots[control_point_index + self.degree + 1];
            let mut derivative = [0.0; 4];
            for coordinate in 0..4 {
                derivative[coordinate] = stable_divided_difference(
                    active[local_index + 1][coordinate],
                    active[local_index][coordinate],
                    self.degree,
                    knot_start,
                    knot_end,
                )?;
            }
            derivative_controls.push(derivative);
        }

        let homogeneous_derivative = de_boor(
            &self.knots[1..self.knots.len() - 1],
            self.degree - 1,
            span - 1,
            parameter,
            derivative_controls.clone(),
        )?;
        let weight = homogeneous[3];
        let weight_derivative = homogeneous_derivative[3];
        let point_coordinates = point.to_array();
        let first_derivative: [Real; 3] = std::array::from_fn(|coordinate| {
            (-point_coordinates[coordinate])
                .mul_add(weight_derivative, homogeneous_derivative[coordinate])
                / weight
        });
        let first_derivative = Vector3::try_from(first_derivative)?;

        if self.degree == 1 {
            return Ok((point, first_derivative, Vector3::try_new(0.0, 0.0, 0.0)?));
        }

        let mut second_derivative_controls = Vec::with_capacity(self.degree - 1);
        for local_index in 0..self.degree - 1 {
            let derivative_control_index = first_control_point + local_index;
            let knot_start = self.knots[derivative_control_index + 2];
            let knot_end = self.knots[derivative_control_index + self.degree + 1];
            let mut derivative = [0.0; 4];
            for coordinate in 0..4 {
                derivative[coordinate] = stable_divided_difference(
                    derivative_controls[local_index + 1][coordinate],
                    derivative_controls[local_index][coordinate],
                    self.degree - 1,
                    knot_start,
                    knot_end,
                )?;
            }
            second_derivative_controls.push(derivative);
        }
        let homogeneous_second_derivative = de_boor(
            &self.knots[2..self.knots.len() - 2],
            self.degree - 2,
            span - 2,
            parameter,
            second_derivative_controls,
        )?;
        let weight_second_derivative = homogeneous_second_derivative[3];
        let first_coordinates = first_derivative.to_array();
        let second_derivative: [Real; 3] = std::array::from_fn(|coordinate| {
            let quotient_terms = (2.0 * weight_derivative).mul_add(
                first_coordinates[coordinate],
                weight_second_derivative * point_coordinates[coordinate],
            );
            (homogeneous_second_derivative[coordinate] - quotient_terms) / weight
        });
        Ok((
            point,
            first_derivative,
            Vector3::try_from(second_derivative)?,
        ))
    }

    pub fn derivative_at(&self, parameter: Real) -> Result<Vector3, GeometryError> {
        self.evaluate_with_derivative(parameter)
            .map(|(_, derivative)| derivative)
    }

    /// Finds the active-domain parameter nearest to a finite model-space
    /// point.
    ///
    /// Every nonempty knot span contributes endpoint and midpoint seeds, with
    /// an additional bounded uniform seed set for high-span and periodic
    /// curves. The best candidates are refined by projected-tangent Newton
    /// steps with clamping and monotone backtracking, so rational and
    /// non-uniform parameterizations do not need to be sampled as polylines.
    pub fn closest_parameter(
        &self,
        target: Point3,
        tolerance: Tolerance,
    ) -> Result<Real, GeometryError> {
        let domain = self.domain();
        let domain_start = *domain.start();
        let domain_end = *domain.end();
        let seeds = curve_closest_parameter_seeds(self.spans(), domain_start, domain_end);
        let mut candidates = seeds
            .into_iter()
            .filter_map(|parameter| {
                self.evaluate(parameter)
                    .and_then(|point| point.distance_to(target))
                    .ok()
                    .map(|distance| (distance, parameter))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
        });
        candidates.truncate(16);
        let mut best = candidates
            .first()
            .copied()
            .ok_or(GeometryError::Degenerate {
                context: "NURBS curve closest-point search",
            })?;
        for (_, seed) in candidates {
            if let Ok((parameter, distance)) =
                self.refine_closest_parameter(target, seed, [domain_start, domain_end], tolerance)
                && (distance < best.0 || (distance == best.0 && parameter < best.1))
            {
                best = (distance, parameter);
            }
        }
        Ok(best.1)
    }

    fn refine_closest_parameter(
        &self,
        target: Point3,
        mut parameter: Real,
        domain: [Real; 2],
        tolerance: Tolerance,
    ) -> Result<(Real, Real), GeometryError> {
        let mut distance = self.evaluate(parameter)?.distance_to(target)?;
        for _ in 0..64 {
            let (point, derivative) = self.evaluate_with_derivative(parameter)?;
            let speed = derivative.length()?;
            if speed == 0.0 {
                break;
            }
            let residual = point.vector_to(target)?;
            let tangent_projection = residual.dot(derivative)? / speed;
            if tangent_projection.abs() <= tolerance.absolute() {
                break;
            }
            let delta = tangent_projection / speed;
            if !delta.is_finite() {
                break;
            }
            let mut step: Real = 1.0;
            let mut accepted = None;
            for _ in 0..24 {
                let candidate = step.mul_add(delta, parameter).clamp(domain[0], domain[1]);
                if candidate == parameter {
                    break;
                }
                let candidate_distance = self.evaluate(candidate)?.distance_to(target)?;
                if candidate_distance <= distance {
                    accepted = Some((candidate, candidate_distance));
                    break;
                }
                step *= 0.5;
            }
            let Some((next_parameter, next_distance)) = accepted else {
                break;
            };
            parameter = next_parameter;
            distance = next_distance;
        }
        Ok((parameter, distance))
    }

    /// Computes arc length span by span with adaptive Gauss-Kronrod
    /// integration of the exact first derivative.
    pub fn length(&self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        let spans = self.spans().collect::<Vec<_>>();
        let absolute_per_span = tolerance.absolute() / spans.len() as Real;
        if absolute_per_span <= 0.0 {
            return Err(GeometryError::NumericalIntegrationDidNotConverge);
        }
        let mut sum = 0.0;
        let mut correction = 0.0;
        for (start, end) in spans {
            let length = integrate_adaptive(
                start,
                end,
                absolute_per_span,
                tolerance.relative(),
                |parameter| self.derivative_at(parameter)?.length(),
            )?;
            let next = sum + length;
            if sum.abs() >= length.abs() {
                correction += (sum - next) + length;
            } else {
                correction += (length - next) + sum;
            }
            sum = next;
        }
        let length = sum + correction;
        require_finite([length], "NURBS curve length")?;
        Ok(length)
    }

    /// Reverses direction by reversing the controls and negating the full
    /// knot vector. A domain `[a, b]` therefore becomes `[-b, -a]`, matching
    /// the OpenNURBS/Rhino convention.
    pub fn reversed(&self) -> Result<Self, GeometryError> {
        let control_points = self.control_points.iter().rev().copied().collect();
        let knots = self.knots.iter().rev().map(|knot| -*knot).collect();
        Self::try_new_rational(self.degree, control_points, knots)
    }

    /// Returns the exact full-knot-vector multiplicity of `parameter`.
    ///
    /// Knot equality is intentionally exact. Near knots remain distinct so a
    /// refinement never changes the caller's requested parameter value.
    pub fn knot_multiplicity(&self, parameter: Real) -> Result<usize, GeometryError> {
        self.validate_parameter(parameter)?;
        Ok(self.knots.iter().filter(|knot| **knot == parameter).count())
    }

    /// Inserts `parameter` until its full-knot-vector multiplicity is at least
    /// `target_multiplicity`, without changing the curve's parameterization or
    /// geometric image.
    ///
    /// A multiplicity of `degree + 1` is supported so callers can represent an
    /// existing discontinuity explicitly. Asking for a multiplicity the curve
    /// already has is a no-op. Refining either active-domain endpoint performs
    /// the equivalent shape-preserving full clamp, because a non-clamped
    /// endpoint is not an ordinary interior knot.
    pub fn try_insert_knot(
        &self,
        parameter: Real,
        target_multiplicity: usize,
    ) -> Result<Self, GeometryError> {
        self.validate_parameter(parameter)?;
        let maximum = self
            .degree
            .checked_add(1)
            .ok_or(GeometryError::InvalidDegree)?;
        if target_multiplicity == 0 || target_multiplicity > maximum {
            return Err(GeometryError::InvalidKnotMultiplicity {
                actual: target_multiplicity,
                maximum,
            });
        }

        let current_multiplicity = self.knot_multiplicity_unchecked(parameter);
        if current_multiplicity >= target_multiplicity {
            return Ok(self.clone());
        }
        let domain = self.domain();
        if parameter == *domain.start() {
            return self.clamped_at_start(parameter);
        }
        if parameter == *domain.end() {
            return self.clamped_at_end(parameter);
        }

        let mut refined = self.clone();
        while refined.knot_multiplicity_unchecked(parameter) < target_multiplicity {
            refined = refined.insert_knot_once(parameter)?;
        }
        Ok(refined)
    }

    /// Splits at a parameter strictly inside the active domain.
    ///
    /// Both results retain the source parameter values and are clamped at
    /// their active ends. At an existing `degree + 1` knot, the independent
    /// left- and right-hand controls remain independent.
    pub fn try_split(&self, parameter: Real) -> Result<(Self, Self), GeometryError> {
        require_finite([parameter], "NURBS curve split parameter")?;
        let domain = self.domain();
        if parameter <= *domain.start() || parameter >= *domain.end() {
            return Err(GeometryError::InvalidCurveSplitParameter);
        }

        let multiplicity = self.knot_multiplicity_unchecked(parameter);
        let refined = if multiplicity < self.degree {
            self.try_insert_knot(parameter, self.degree)?
        } else {
            self.clone()
        };
        let multiplicity = refined.knot_multiplicity_unchecked(parameter);
        let first_knot = refined.knots.partition_point(|knot| *knot < parameter);
        let after_knots = refined.knots.partition_point(|knot| *knot <= parameter);

        let (left_controls, left_knots, right_controls, right_knots) =
            if multiplicity == self.degree + 1 {
                (
                    refined.control_points[..first_knot].to_vec(),
                    refined.knots[..after_knots].to_vec(),
                    refined.control_points[first_knot..].to_vec(),
                    refined.knots[first_knot..].to_vec(),
                )
            } else {
                debug_assert_eq!(multiplicity, self.degree);
                let shared_control = after_knots - self.degree - 1;
                let mut left_knots = refined.knots[..after_knots].to_vec();
                left_knots.push(parameter);
                let mut right_knots =
                    Vec::with_capacity(refined.knots.len() - (shared_control + 1) + 1);
                right_knots.push(parameter);
                right_knots.extend_from_slice(&refined.knots[shared_control + 1..]);
                (
                    refined.control_points[..=shared_control].to_vec(),
                    left_knots,
                    refined.control_points[shared_control..].to_vec(),
                    right_knots,
                )
            };

        let left = Self::try_new_rational(self.degree, left_controls, left_knots)?
            .clamped_to_active_domain()?;
        let right = Self::try_new_rational(self.degree, right_controls, right_knots)?
            .clamped_to_active_domain()?;
        Ok((left, right))
    }

    /// Extracts an exact subcurve while retaining the source parameter values.
    /// Partial trims are clamped at both active ends. Trimming to the exact
    /// existing domain is a no-op, preserving periodic form.
    pub fn try_trimmed(&self, interval: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        let start = *interval.start();
        let end = *interval.end();
        if !start.is_finite() || !end.is_finite() || start >= end {
            return Err(GeometryError::InvalidCurveTrimInterval);
        }
        let domain = self.domain();
        if start < *domain.start() || end > *domain.end() {
            return Err(GeometryError::InvalidCurveTrimInterval);
        }
        if start == *domain.start() && end == *domain.end() {
            return Ok(self.clone());
        }

        let after_start = if start == *domain.start() {
            self.clone()
        } else {
            self.try_split(start)?.1
        };
        if end == *domain.end() {
            Ok(after_start)
        } else {
            Ok(after_start.try_split(end)?.0)
        }
    }

    pub fn transformed(&self, transform: AffineTransform3) -> Result<Self, GeometryError> {
        let control_points = self
            .control_points
            .iter()
            .map(|control_point| {
                Ok(WeightedPoint3 {
                    point: transform.transform_point(control_point.point)?,
                    weight: control_point.weight,
                })
            })
            .collect::<Result<_, GeometryError>>()?;
        Self::try_new_rational(self.degree, control_points, self.knots.clone())
    }

    fn checked_span(&self, parameter: Real) -> Result<usize, GeometryError> {
        self.validate_parameter(parameter)?;
        Ok(self.find_span(parameter))
    }

    fn validate_parameter(&self, parameter: Real) -> Result<(), GeometryError> {
        require_finite([parameter], "NURBS parameter")?;
        let domain = self.domain();
        let domain_start = *domain.start();
        let domain_end = *domain.end();
        if parameter < domain_start || parameter > domain_end {
            return Err(GeometryError::ParameterOutOfDomain {
                parameter,
                domain_start,
                domain_end,
            });
        }
        Ok(())
    }

    fn knot_multiplicity_unchecked(&self, parameter: Real) -> usize {
        self.knots.iter().filter(|knot| **knot == parameter).count()
    }

    fn insert_knot_once(&self, parameter: Real) -> Result<Self, GeometryError> {
        let control_count = self.control_points.len();
        let last_control = control_count - 1;
        debug_assert!(parameter > self.knots[self.degree]);
        debug_assert!(parameter < self.knots[control_count]);
        let span = self.knots.partition_point(|knot| *knot <= parameter) - 1;
        let multiplicity = self.knot_multiplicity_unchecked(parameter);
        debug_assert!(multiplicity <= self.degree);
        debug_assert!(span >= self.degree && span <= last_control);

        let first_unchanged = span - self.degree;
        let first_shifted = span - multiplicity + 1;
        let mut controls = Vec::with_capacity(control_count + 1);
        for new_index in 0..=control_count {
            let control = if new_index <= first_unchanged {
                self.control_points[new_index]
            } else if new_index < first_shifted {
                let denominator_start = self.knots[new_index];
                let denominator_end = self.knots[new_index + self.degree];
                let alpha = interval_fraction(parameter, denominator_start, denominator_end)?;
                blend_weighted_control_points(
                    self.control_points[new_index - 1],
                    self.control_points[new_index],
                    alpha,
                )?
            } else {
                self.control_points[new_index - 1]
            };
            controls.push(control);
        }

        let mut knots = Vec::with_capacity(self.knots.len() + 1);
        knots.extend_from_slice(&self.knots[..=span]);
        knots.push(parameter);
        knots.extend_from_slice(&self.knots[span + 1..]);
        Self::try_new_rational(self.degree, controls, knots)
    }

    fn clamped_to_active_domain(&self) -> Result<Self, GeometryError> {
        let start = *self.domain().start();
        let end = *self.domain().end();
        self.clamped_at_start(start)?.clamped_at_end(end)
    }

    fn clamped_at_start(&self, start: Real) -> Result<Self, GeometryError> {
        if self.knots[..=self.degree].iter().all(|knot| *knot == start) {
            return Ok(self.clone());
        }

        let span = self.find_span(start);
        let (_, right_controls) = self.de_boor_side_controls(span, start)?;
        let mut controls = Vec::with_capacity(self.control_points.len() - (span - self.degree));
        controls.extend(right_controls);
        controls.extend_from_slice(&self.control_points[span + 1..]);
        let mut knots = Vec::with_capacity(controls.len() + self.degree + 1);
        knots.resize(self.degree + 1, start);
        knots.extend_from_slice(&self.knots[span + 1..]);
        Self::try_new_rational(self.degree, controls, knots)
    }

    fn clamped_at_end(&self, end: Real) -> Result<Self, GeometryError> {
        if self.knots[self.knots.len() - self.degree - 1..]
            .iter()
            .all(|knot| *knot == end)
        {
            return Ok(self.clone());
        }

        let span = self.find_span(end);
        let (left_controls, _) = self.de_boor_side_controls(span, end)?;
        let first_active_control = span - self.degree;
        let mut controls = Vec::with_capacity(span + 1);
        controls.extend_from_slice(&self.control_points[..first_active_control]);
        controls.extend(left_controls);
        let mut knots = Vec::with_capacity(controls.len() + self.degree + 1);
        knots.extend_from_slice(&self.knots[..=span]);
        knots.resize(knots.len() + self.degree + 1, end);
        Self::try_new_rational(self.degree, controls, knots)
    }

    fn de_boor_side_controls(
        &self,
        span: usize,
        parameter: Real,
    ) -> Result<(Vec<WeightedPoint3>, Vec<WeightedPoint3>), GeometryError> {
        let mut work = self.control_points[span - self.degree..=span].to_vec();
        let mut left = Vec::with_capacity(self.degree + 1);
        let mut right = Vec::with_capacity(self.degree + 1);
        left.push(work[0]);
        right.push(work[self.degree]);
        for level in 1..=self.degree {
            for local_index in (level..=self.degree).rev() {
                let knot_index = span - self.degree + local_index;
                let alpha = interval_fraction(
                    parameter,
                    self.knots[knot_index],
                    self.knots[knot_index + self.degree - level + 1],
                )?;
                work[local_index] =
                    blend_weighted_control_points(work[local_index - 1], work[local_index], alpha)?;
            }
            left.push(work[level]);
            right.push(work[self.degree]);
        }
        right.reverse();
        Ok((left, right))
    }

    fn active_homogeneous_control_points(
        &self,
        span: usize,
    ) -> Result<Vec<[Real; 4]>, GeometryError> {
        let first_control_point = span - self.degree;
        let active = &self.control_points[first_control_point..=span];
        let weight_scale = active
            .iter()
            .map(|control_point| control_point.weight)
            .fold(0.0, Real::max);
        let mut homogeneous = Vec::with_capacity(active.len());
        for control_point in active {
            let weight = control_point.weight / weight_scale;
            let point = control_point.point;
            let value = [
                point.x() * weight,
                point.y() * weight,
                point.z() * weight,
                weight,
            ];
            require_finite(value, "homogeneous NURBS control point")?;
            homogeneous.push(value);
        }
        Ok(homogeneous)
    }

    fn find_span(&self, parameter: Real) -> usize {
        find_span_in_knots(
            &self.knots,
            self.degree,
            self.control_points.len(),
            parameter,
        )
    }
}

fn vector_divided_by(vector: Vector3, divisor: Real) -> Result<Vector3, GeometryError> {
    require_finite([divisor], "vector divisor")?;
    if divisor == 0.0 {
        return Err(GeometryError::Degenerate { context: "vector" });
    }
    Vector3::try_new(
        vector.x() / divisor,
        vector.y() / divisor,
        vector.z() / divisor,
    )
}

fn add_vectors(left: Vector3, right: Vector3) -> Result<Vector3, GeometryError> {
    Vector3::try_new(
        left.x() + right.x(),
        left.y() + right.y(),
        left.z() + right.z(),
    )
}

fn subtract_scaled_vector(
    vector: Vector3,
    direction: Vector3,
    scale: Real,
) -> Result<Vector3, GeometryError> {
    require_finite([scale], "vector projection")?;
    Vector3::try_new(
        (-scale).mul_add(direction.x(), vector.x()),
        (-scale).mul_add(direction.y(), vector.y()),
        (-scale).mul_add(direction.z(), vector.z()),
    )
}

fn quadratic_power_point(
    origin: Point3,
    linear: Vector3,
    quadratic: Vector3,
    parameter: Real,
) -> Result<Point3, GeometryError> {
    require_finite([parameter], "quadratic parabola parameter")?;
    let squared = parameter * parameter;
    Point3::try_new(
        quadratic
            .x()
            .mul_add(squared, linear.x().mul_add(parameter, origin.x())),
        quadratic
            .y()
            .mul_add(squared, linear.y().mul_add(parameter, origin.y())),
        quadratic
            .z()
            .mul_add(squared, linear.z().mul_add(parameter, origin.z())),
    )
}

fn quadratic_parabola_from_second_difference(
    start: Point3,
    end: Point3,
    quadratic: Vector3,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let midpoint = Point3::try_new(
        0.5 * start.x() + 0.5 * end.x(),
        0.5 * start.y() + 0.5 * end.y(),
        0.5 * start.z() + 0.5 * end.z(),
    )?;
    let middle = Point3::try_new(
        (-0.5_f64).mul_add(quadratic.x(), midpoint.x()),
        (-0.5_f64).mul_add(quadratic.y(), midpoint.y()),
        (-0.5_f64).mul_add(quadratic.z(), midpoint.z()),
    )?;
    let curve = NurbsCurve::try_new(
        2,
        vec![start, middle, end],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )?;
    curve.try_parabola_focus(tolerance)?;
    Ok(curve)
}

pub(crate) fn control_polygon_length(control_points: &[Point3]) -> Result<Real, GeometryError> {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for pair in control_points.windows(2) {
        let length = pair[0].distance_to(pair[1])?;
        let next = sum + length;
        if sum.abs() >= length.abs() {
            correction += (sum - next) + length;
        } else {
            correction += (length - next) + sum;
        }
        sum = next;
    }
    let length = sum + correction;
    require_finite([length], "control polygon length")?;
    Ok(length)
}

fn reparameterize_value(
    value: Real,
    source_start: Real,
    source_end: Real,
    target_start: Real,
    target_end: Real,
) -> Result<Real, GeometryError> {
    let source_scale = value.abs().max(source_start.abs()).max(source_end.abs());
    debug_assert!(source_scale > 0.0);
    let scaled_source_start = source_start / source_scale;
    let source_span = source_end / source_scale - scaled_source_start;
    if !source_span.is_finite() || source_span <= 0.0 {
        return Err(GeometryError::InvalidKnotVector {
            context: "the source domain cannot be stably reparameterized",
        });
    }
    let normalized = (value / source_scale - scaled_source_start) / source_span;
    if !normalized.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "NURBS reparameterized knot",
        });
    }

    let target_scale = target_start.abs().max(target_end.abs());
    debug_assert!(target_scale > 0.0);
    let scaled_target_start = target_start / target_scale;
    let scaled_target_span = target_end / target_scale - scaled_target_start;
    let mapped = normalized.mul_add(scaled_target_span, scaled_target_start) * target_scale;
    require_finite([mapped], "NURBS reparameterized knot")?;
    Ok(mapped)
}

fn clamped_control_point_curve(
    degree: usize,
    control_points: Vec<Point3>,
) -> Result<NurbsCurve, GeometryError> {
    let domain_end = control_polygon_length(&control_points)?;
    if domain_end <= 0.0 {
        return Err(GeometryError::Degenerate {
            context: "control-point curve",
        });
    }
    let knots = clamped_uniform_knots(degree, control_points.len())?
        .into_iter()
        .map(|knot| knot * domain_end)
        .collect();
    NurbsCurve::try_new(degree, control_points, knots)
}

fn periodic_control_point_curve(
    degree: usize,
    unique_control_points: Vec<Point3>,
) -> Result<NurbsCurve, GeometryError> {
    let unique_count = unique_control_points.len();
    let control_count =
        unique_count
            .checked_add(degree)
            .ok_or(GeometryError::InvalidKnotVector {
                context: "periodic control-point count overflowed usize",
            })?;
    let lead_count = (degree - 1) / 2;
    let tail_count = degree - lead_count;
    let mut control_points = Vec::with_capacity(control_count);
    control_points.extend_from_slice(&unique_control_points[unique_count - lead_count..]);
    control_points.extend_from_slice(&unique_control_points);
    control_points.extend_from_slice(&unique_control_points[..tail_count]);

    let domain_end = control_polygon_length(&control_points)?;
    if domain_end <= 0.0 {
        return Err(GeometryError::Degenerate {
            context: "control-point curve",
        });
    }
    let step = domain_end / unique_count as Real;
    require_finite([step], "periodic control-point curve knot step")?;
    let knot_count = control_count
        .checked_add(degree)
        .and_then(|count| count.checked_add(1))
        .ok_or(GeometryError::InvalidKnotVector {
            context: "periodic control-point curve knot count overflowed usize",
        })?;
    let knots = (0..knot_count)
        .map(|index| (index as Real - degree as Real) * step)
        .collect::<Vec<_>>();
    require_finite(knots.iter().copied(), "periodic control-point curve knots")?;
    NurbsCurve::try_new(degree, control_points, knots)
}

pub(crate) fn bspline_basis_values(
    knots: &[Real],
    degree: usize,
    control_count: usize,
    parameter: Real,
) -> Result<Vec<Real>, GeometryError> {
    debug_assert_eq!(knots.len(), control_count + degree + 1);
    debug_assert!(degree >= 1 && control_count > degree);
    let domain_start = knots[degree];
    let domain_end = knots[control_count];
    require_finite([parameter], "B-spline basis parameter")?;
    if parameter < domain_start || parameter > domain_end {
        return Err(GeometryError::ParameterOutOfDomain {
            parameter,
            domain_start,
            domain_end,
        });
    }
    let span = find_span_in_knots(knots, degree, control_count, parameter);
    let mut local = vec![0.0; degree + 1];
    local[0] = 1.0;
    for column in 1..=degree {
        let mut saved = 0.0;
        for row in 0..column {
            let left_knot = knots[span + 1 - column + row];
            let right_knot = knots[span + row + 1];
            let left_fraction = interval_fraction(parameter, left_knot, right_knot)?;
            let value = local[row];
            local[row] = (1.0 - left_fraction).mul_add(value, saved);
            saved = left_fraction * value;
        }
        local[column] = saved;
    }
    require_finite(local.iter().copied(), "B-spline basis")?;

    let mut values = vec![0.0; control_count];
    values[span - degree..=span].copy_from_slice(&local);
    Ok(values)
}

pub(crate) fn find_span_in_knots(
    knots: &[Real],
    degree: usize,
    control_count: usize,
    parameter: Real,
) -> usize {
    let last_control_point = control_count - 1;
    if parameter >= knots[control_count] {
        // Select the nonempty span immediately to the left. This also handles
        // valid full vectors whose equal end knots straddle the active-domain
        // index instead of occupying only the exterior tail.
        return (knots.partition_point(|knot| *knot < knots[control_count]) - 1)
            .clamp(degree, last_control_point);
    }
    if parameter <= knots[degree] {
        // Select the nonempty span immediately to the right. A clamped vector
        // still returns `degree`; a refined non-clamped start can have more
        // equal knots after that index.
        return (knots.partition_point(|knot| *knot <= knots[degree]) - 1)
            .clamp(degree, last_control_point);
    }

    let mut low = degree;
    let mut high = control_count;
    let mut middle = (low + high) / 2;
    while parameter < knots[middle] || parameter >= knots[middle + 1] {
        if parameter < knots[middle] {
            high = middle;
        } else {
            low = middle;
        }
        middle = (low + high) / 2;
    }
    middle
}

pub(crate) fn curve_points_coincident(left: Point3, right: Point3) -> bool {
    left.to_array()
        .into_iter()
        .zip(right.to_array())
        .all(|(left, right)| {
            let difference = (left - right).abs();
            difference <= CURVE_COINCIDENCE_ABSOLUTE
                || difference <= (left.abs() + right.abs()) * CURVE_COINCIDENCE_RELATIVE
        })
}

/// Returns the half-open raw-control window used by OpenNURBS control
/// polygons. Periodic directions keep one closing control and slide the
/// window until its endpoint Greville abscissae bracket the active domain.
pub(crate) fn control_polygon_range(
    degree: usize,
    control_count: usize,
    knots: &[Real],
    periodic: bool,
) -> (usize, usize) {
    debug_assert!(degree >= 1);
    debug_assert_eq!(knots.len(), control_count + degree + 1);
    if !periodic {
        return (0, control_count);
    }

    let greville = |control: usize| {
        let values = &knots[control + 1..=control + degree];
        let scale = values.iter().map(|value| value.abs()).fold(0.0, Real::max);
        if scale == 0.0 {
            0.0
        } else {
            (values.iter().map(|value| value / scale).sum::<Real>() / degree as Real)
                .clamp(-1.0, 1.0)
                * scale
        }
    };
    let domain_start = knots[degree];
    let domain_end = knots[control_count];
    let mut start = 0;
    let mut end = control_count - (degree - 1);
    while end < control_count && greville(start) < domain_start && greville(end - 1) <= domain_end {
        start += 1;
        end += 1;
    }
    (start, end)
}

pub(crate) fn knot_vector_is_periodic(order: usize, control_count: usize, knots: &[Real]) -> bool {
    if order < 2 || control_count < order || knots.len() != order + control_count - 2 {
        return false;
    }
    if order == 2 {
        return false;
    }
    if (order <= 4 && control_count < order + 2) || (order > 4 && control_count < 2 * order - 2) {
        return false;
    }

    let mut tolerance = (knots[order - 1] - knots[order - 3]).abs() * OPENNURBS_SQRT_EPSILON;
    tolerance =
        tolerance.max((knots[control_count - 1] - knots[order - 2]).abs() * OPENNURBS_SQRT_EPSILON);
    let right_start = control_count - order + 1;
    (0..2 * (order - 2)).all(|index| {
        let left_delta = knots[index + 1] - knots[index];
        let right_delta = knots[right_start + index + 1] - knots[right_start + index];
        (left_delta - right_delta).abs() <= tolerance
    })
}

pub(crate) fn de_boor<const DIMENSION: usize>(
    knots: &[Real],
    degree: usize,
    span: usize,
    parameter: Real,
    mut work: Vec<[Real; DIMENSION]>,
) -> Result<[Real; DIMENSION], GeometryError> {
    debug_assert_eq!(work.len(), degree + 1);
    for level in 1..=degree {
        for local_index in (level..=degree).rev() {
            let knot_index = span - degree + local_index;
            let left_knot = knots[knot_index];
            let right_knot = knots[knot_index + degree - level + 1];
            let alpha = interval_fraction(parameter, left_knot, right_knot)?;
            work[local_index] = blend_homogeneous(work[local_index - 1], work[local_index], alpha)?;
        }
    }
    Ok(work[degree])
}

pub(crate) fn project_homogeneous(homogeneous: [Real; 4]) -> Result<Point3, GeometryError> {
    let weight = homogeneous[3];
    if !weight.is_finite() || weight <= 0.0 {
        return Err(GeometryError::ZeroWeightAtParameter);
    }
    Point3::try_new(
        homogeneous[0] / weight,
        homogeneous[1] / weight,
        homogeneous[2] / weight,
    )
}

pub(crate) fn stable_divided_difference(
    right: Real,
    left: Real,
    degree: usize,
    interval_start: Real,
    interval_end: Real,
) -> Result<Real, GeometryError> {
    if right == left {
        return Ok(0.0);
    }

    let direct_numerator = right - left;
    let (numerator, numerator_factor) = if direct_numerator.is_finite() {
        (direct_numerator, 1.0)
    } else {
        (right * 0.5 - left * 0.5, 2.0)
    };
    let direct_denominator = interval_end - interval_start;
    let (denominator, denominator_factor) = if direct_denominator.is_finite() {
        (direct_denominator, 1.0)
    } else {
        (interval_end * 0.5 - interval_start * 0.5, 2.0)
    };
    if !numerator.is_finite() || !denominator.is_finite() || denominator <= 0.0 || numerator == 0.0
    {
        return Err(GeometryError::NonFinite {
            context: "NURBS derivative divided difference",
        });
    }

    let multiplier = degree as Real * numerator_factor / denominator_factor;
    let candidates = [
        (numerator / denominator) * multiplier,
        (numerator * multiplier) / denominator,
    ];
    if let Some(value) = candidates
        .into_iter()
        .find(|value| value.is_finite() && *value != 0.0)
    {
        Ok(value)
    } else if candidates.into_iter().any(|value| value == 0.0) {
        Ok(0.0)
    } else {
        Err(GeometryError::NonFinite {
            context: "NURBS derivative divided difference",
        })
    }
}

pub(crate) fn interval_fraction(
    value: Real,
    interval_start: Real,
    interval_end: Real,
) -> Result<Real, GeometryError> {
    let denominator = interval_end - interval_start;
    let alpha = if denominator.is_finite() && denominator > 0.0 {
        (value - interval_start) / denominator
    } else if denominator.is_infinite() && interval_start < interval_end {
        // Halving preserves the ratio when subtracting opposite, very large
        // finite endpoints would overflow.
        let scaled_start = interval_start * 0.5;
        (value * 0.5 - scaled_start) / (interval_end * 0.5 - scaled_start)
    } else {
        return Err(GeometryError::InvalidKnotVector {
            context: "an active de Boor knot interval is empty",
        });
    };
    if !alpha.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "de Boor blend factor",
        });
    }
    // The validated span brackets `value`; a value just outside this range can
    // only be floating-point roundoff in the ratio calculation.
    Ok(alpha.clamp(0.0, 1.0))
}

fn validate_structure(
    degree: usize,
    control_points: &[WeightedPoint3],
    knots: &[Real],
) -> Result<(), GeometryError> {
    validate_direction(degree, control_points.len(), knots)?;
    for (index, control_point) in control_points.iter().enumerate() {
        if !control_point.weight.is_finite() || control_point.weight <= 0.0 {
            return Err(GeometryError::InvalidWeight { index });
        }
    }

    Ok(())
}

fn curve_closest_parameter_seeds(
    spans: impl Iterator<Item = (Real, Real)>,
    domain_start: Real,
    domain_end: Real,
) -> Vec<Real> {
    const UNIFORM_SEED_COUNT: usize = 33;
    let mut seeds = Vec::new();
    for (start, end) in spans {
        seeds.extend([start, start * 0.5 + end * 0.5, end]);
    }
    for index in 0..UNIFORM_SEED_COUNT {
        let fraction = index as Real / (UNIFORM_SEED_COUNT - 1) as Real;
        seeds.push(domain_start.mul_add(1.0 - fraction, domain_end * fraction));
    }
    seeds.sort_by(Real::total_cmp);
    seeds.dedup();
    seeds
}

pub(crate) fn validate_direction(
    degree: usize,
    control_point_count: usize,
    knots: &[Real],
) -> Result<(), GeometryError> {
    validate_control_point_count(degree, control_point_count)?;
    let expected_knot_count = control_point_count
        .checked_add(degree)
        .and_then(|count| count.checked_add(1))
        .ok_or(GeometryError::InvalidKnotVector {
            context: "knot count overflowed usize",
        })?;
    if knots.len() != expected_knot_count {
        return Err(GeometryError::InvalidKnotCount {
            expected: expected_knot_count,
            actual: knots.len(),
        });
    }
    if !knots.iter().all(|knot| knot.is_finite()) {
        return Err(GeometryError::InvalidKnotVector {
            context: "knots must be finite",
        });
    }
    if knots.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(GeometryError::InvalidKnotVector {
            context: "knots must be nondecreasing",
        });
    }
    if knots[degree] >= knots[control_point_count] {
        return Err(GeometryError::InvalidKnotVector {
            context: "the active domain must have positive length",
        });
    }

    let maximum_multiplicity = degree + 1;
    let mut run_length = 1;
    for pair in knots.windows(2) {
        if pair[0] == pair[1] {
            run_length += 1;
            if run_length > maximum_multiplicity {
                return Err(GeometryError::InvalidKnotVector {
                    context: "knot multiplicity exceeds degree plus one",
                });
            }
        } else {
            run_length = 1;
        }
    }

    Ok(())
}

pub(crate) fn clamped_uniform_knots(
    degree: usize,
    control_point_count: usize,
) -> Result<Vec<Real>, GeometryError> {
    validate_control_point_count(degree, control_point_count)?;
    let knot_count = control_point_count
        .checked_add(degree)
        .and_then(|count| count.checked_add(1))
        .ok_or(GeometryError::InvalidKnotVector {
            context: "knot count overflowed usize",
        })?;
    let span_count = control_point_count - degree;
    Ok((0..knot_count)
        .map(|index| {
            if index <= degree {
                0.0
            } else if index >= control_point_count {
                1.0
            } else {
                (index - degree) as Real / span_count as Real
            }
        })
        .collect())
}

fn validate_control_point_count(degree: usize, count: usize) -> Result<(), GeometryError> {
    if degree == 0 {
        return Err(GeometryError::InvalidDegree);
    }
    let required = degree.checked_add(1).ok_or(GeometryError::InvalidDegree)?;
    if count < required {
        return Err(GeometryError::InsufficientControlPoints {
            degree,
            required,
            actual: count,
        });
    }
    Ok(())
}

fn blend_homogeneous<const DIMENSION: usize>(
    left: [Real; DIMENSION],
    right: [Real; DIMENSION],
    alpha: Real,
) -> Result<[Real; DIMENSION], GeometryError> {
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
        return Err(GeometryError::InvalidKnotVector {
            context: "de Boor blend factor is outside zero to one",
        });
    }
    let complement = 1.0 - alpha;
    let result = std::array::from_fn(|index| left[index].mul_add(complement, right[index] * alpha));
    require_finite(result, "homogeneous NURBS evaluation")?;
    Ok(result)
}

fn blend_weighted_control_points(
    left: WeightedPoint3,
    right: WeightedPoint3,
    alpha: Real,
) -> Result<WeightedPoint3, GeometryError> {
    if alpha == 0.0 {
        return Ok(left);
    }
    if alpha == 1.0 {
        return Ok(right);
    }
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
        return Err(GeometryError::InvalidKnotVector {
            context: "knot-insertion blend factor is outside zero to one",
        });
    }

    // Work with weights normalized by their local maximum. This avoids the
    // overflow in `weight * coordinate` that a literal homogeneous blend can
    // encounter, while producing the identical projective control point.
    let scale = left.weight.max(right.weight);
    let left_weight = left.weight / scale;
    let right_weight = right.weight / scale;
    let complement = 1.0 - alpha;
    let normalized_weight = left_weight.mul_add(complement, right_weight * alpha);
    if !normalized_weight.is_finite() || normalized_weight <= 0.0 {
        return Err(GeometryError::NonFinite {
            context: "knot-insertion control weight",
        });
    }
    let weight = normalized_weight * scale;
    if !weight.is_finite() || weight <= 0.0 {
        return Err(GeometryError::NonFinite {
            context: "knot-insertion control weight",
        });
    }

    let right_fraction = ((right_weight * alpha) / normalized_weight).clamp(0.0, 1.0);
    let left_coordinates = left.point.to_array();
    let right_coordinates = right.point.to_array();
    let point: [Real; 3] = std::array::from_fn(|index| {
        left_coordinates[index].mul_add(
            1.0 - right_fraction,
            right_coordinates[index] * right_fraction,
        )
    });
    require_finite(point, "knot-insertion control point")?;
    Ok(WeightedPoint3 {
        point: Point3::try_new(point[0], point[1], point[2])?,
        weight,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tolerance;

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    fn assert_point_near(actual: Point3, expected: Point3) {
        assert!(
            actual.is_near(
                expected,
                Tolerance::try_new(1.0e-12, 1.0e-12, 1.0e-12).unwrap()
            ),
            "actual {actual:?}, expected {expected:?}"
        );
    }

    #[test]
    fn degree_one_curve_interpolates_linearly() {
        let curve = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(2.0, 4.0)],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        assert_eq!(curve.evaluate(0.0).unwrap(), point(0.0, 0.0));
        assert_eq!(curve.evaluate(1.0).unwrap(), point(2.0, 4.0));
        assert_eq!(curve.evaluate(0.25).unwrap(), point(0.5, 1.0));
        assert_eq!(
            curve.derivative_at(0.25).unwrap(),
            Vector3::try_new(2.0, 4.0, 0.0).unwrap()
        );
    }

    #[test]
    fn conic_matches_rhino_rho_weight_and_normalized_layout() {
        let start = point(0.0, 0.0);
        let apex = point(5.0, 5.0);
        let end = point(10.0, 0.0);

        for (rho, expected_weight, rational) in [
            (0.25, 1.0 / 3.0, true),
            (0.5, 1.0, false),
            (0.75, 3.0, true),
        ] {
            let curve = NurbsCurve::try_conic(start, apex, end, rho).unwrap();
            assert_eq!(curve.degree(), 2);
            assert_eq!(curve.is_rational(), rational);
            assert_eq!(curve.domain(), 0.0..=1.0);
            assert_eq!(curve.knots(), &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
            assert_eq!(curve.control_points()[0].point(), start);
            assert_eq!(curve.control_points()[0].weight(), 1.0);
            assert_eq!(curve.control_points()[1].point(), apex);
            assert_eq!(curve.control_points()[1].weight(), expected_weight);
            assert_eq!(curve.control_points()[2].point(), end);
            assert_eq!(curve.control_points()[2].weight(), 1.0);
        }
    }

    #[test]
    fn conic_through_point_projects_to_plane_and_recovers_weight() {
        let start = point(0.0, 0.0);
        let apex = point(5.0, 5.0);
        let end = point(10.0, 0.0);
        let through = Point3::try_new(5.0, 2.0, 37.0).unwrap();
        let curve =
            NurbsCurve::try_conic_through_point(start, apex, end, through, Tolerance::DEFAULT)
                .unwrap();
        assert!((curve.control_points()[1].weight() - 2.0 / 3.0).abs() < 1.0e-15);
        assert_point_near(curve.evaluate(0.5).unwrap(), point(5.0, 2.0));

        let start = Point3::try_new(1.0, 2.0, 3.0).unwrap();
        let apex = Point3::try_new(4.0, 9.0, 7.0).unwrap();
        let end = Point3::try_new(9.0, 4.0, 5.0).unwrap();
        let through = Point3::try_new(
            3.666_666_666_666_666_5,
            5.047_619_047_619_047_4,
            4.904_761_904_761_905,
        )
        .unwrap();
        let curve =
            NurbsCurve::try_conic_through_point(start, apex, end, through, Tolerance::DEFAULT)
                .unwrap();
        assert!((curve.control_points()[1].weight() - 2.0 / 3.0).abs() < 1.0e-14);
        assert_point_near(curve.evaluate(0.4).unwrap(), through);

        let closed_cusp = NurbsCurve::try_conic_through_point(
            point(0.0, 0.0),
            point(5.0, 5.0),
            point(0.0, 0.0),
            point(2.0, 2.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!((closed_cusp.control_points()[1].weight() - 2.0 / 3.0).abs() < 1.0e-15);
        assert_point_near(closed_cusp.evaluate(0.5).unwrap(), point(2.0, 2.0));
    }

    #[test]
    fn conic_accepts_degenerate_rho_controls_and_rejects_invalid_through_points() {
        let start = point(0.0, 0.0);
        let apex = point(5.0, 5.0);
        let end = point(10.0, 0.0);
        for rho in [-1.0, 0.0, 1.0, Real::INFINITY, Real::NAN] {
            assert!(NurbsCurve::try_conic(start, apex, end, rho).is_err());
        }
        let collinear = NurbsCurve::try_conic(start, point(5.0, 0.0), end, 0.5).unwrap();
        assert_eq!(collinear.control_points()[1].point(), point(5.0, 0.0));
        let coincident = NurbsCurve::try_conic(start, start, start, 0.5).unwrap();
        assert!(
            coincident
                .control_points()
                .iter()
                .all(|control| control.point() == start)
        );
        for through in [
            point(5.0, 0.0),
            point(5.0, 5.0),
            point(5.0, 6.0),
            point(-1.0, 1.0),
        ] {
            assert!(
                NurbsCurve::try_conic_through_point(start, apex, end, through, Tolerance::DEFAULT,)
                    .is_err()
            );
        }
    }

    #[test]
    fn parabola_matches_rhino_full_and_half_quadratic_layouts() {
        let frame = Frame3::try_from_directions(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();

        let full = NurbsCurve::try_parabola(frame, 2.0, 1.0, false).unwrap();
        assert_eq!(full.degree(), 2);
        assert!(!full.is_rational());
        assert_eq!(full.domain(), 0.0..=1.0);
        assert_eq!(full.knots(), &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        assert_eq!(
            full.control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            vec![
                Point3::try_new(1.0, 0.0, 4.0).unwrap(),
                Point3::try_new(1.0, 2.0, 2.0).unwrap(),
                Point3::try_new(1.0, 4.0, 4.0).unwrap(),
            ]
        );
        assert_eq!(full.evaluate(0.5).unwrap(), frame.origin());

        let half = NurbsCurve::try_parabola(frame, 2.0, 1.0, true).unwrap();
        assert_eq!(
            half.control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            vec![
                Point3::try_new(1.0, 2.0, 3.0).unwrap(),
                Point3::try_new(1.0, 3.0, 3.0).unwrap(),
                Point3::try_new(1.0, 4.0, 4.0).unwrap(),
            ]
        );
        for parameter in [0.0, 0.125, 0.5, 1.0] {
            assert_point_near(
                half.evaluate(parameter).unwrap(),
                Point3::try_new(1.0, 2.0 + 2.0 * parameter, 3.0 + parameter * parameter).unwrap(),
            );
        }

        assert!(matches!(
            NurbsCurve::try_parabola(frame, 0.0, 1.0, false),
            Err(GeometryError::Degenerate {
                context: "parabola"
            })
        ));
        assert!(matches!(
            NurbsCurve::try_parabola(frame, 1.0, -1.0, true),
            Err(GeometryError::Degenerate {
                context: "parabola"
            })
        ));
        assert!(matches!(
            NurbsCurve::try_parabola(frame, f64::INFINITY, 1.0, false),
            Err(GeometryError::NonFinite { .. })
        ));
    }

    #[test]
    fn three_point_vertex_parabola_matches_rhino_and_reverses_exactly() {
        let vertex = Point3::try_new(1.0, 2.0, 3.0).unwrap();
        let start = Point3::try_new(-2.0, 5.0, 7.0).unwrap();
        let end = Point3::try_new(8.0, 4.0, 6.0).unwrap();
        let curve =
            NurbsCurve::try_parabola_from_vertex(vertex, start, end, Tolerance::DEFAULT).unwrap();
        assert_eq!(curve.degree(), 2);
        assert!(!curve.is_rational());
        assert_eq!(curve.domain(), 0.0..=1.0);
        assert_eq!(curve.knots(), &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        assert_eq!(curve.control_points()[0].point(), start);
        assert_point_near(
            curve.control_points()[1].point(),
            Point3::try_new(
                -0.011_269_980_002_004_854,
                -0.655_652_364_729_298_2,
                -0.676_681_758_332_815_5,
            )
            .unwrap(),
        );
        assert_eq!(curve.control_points()[2].point(), end);
        assert_point_near(curve.evaluate(0.448_996_842_378_081_96).unwrap(), vertex);

        let reversed =
            NurbsCurve::try_parabola_from_vertex(vertex, end, start, Tolerance::DEFAULT).unwrap();
        for (actual, expected) in reversed
            .control_points()
            .iter()
            .zip(curve.control_points().iter().rev())
        {
            assert_point_near(actual.point(), expected.point());
        }
    }

    #[test]
    fn three_point_focus_parabola_matches_rhino_and_recovers_focus() {
        let focus = Point3::try_new(1.0, 2.0, 3.0).unwrap();
        let start = Point3::try_new(-2.0, 5.0, 7.0).unwrap();
        let end = Point3::try_new(8.0, 4.0, 6.0).unwrap();
        let curve =
            NurbsCurve::try_parabola_from_focus(focus, start, end, Tolerance::DEFAULT).unwrap();
        assert_eq!(curve.control_points()[0].point(), start);
        assert_point_near(
            curve.control_points()[1].point(),
            Point3::try_new(
                -0.856_025_073_925_939_4,
                -1.808_620_322_768_226_7,
                -2.287_962_111_716_675_3,
            )
            .unwrap(),
        );
        assert_eq!(curve.control_points()[2].point(), end);
        assert_point_near(curve.try_parabola_focus(Tolerance::DEFAULT).unwrap(), focus);

        let reversed =
            NurbsCurve::try_parabola_from_focus(focus, end, start, Tolerance::DEFAULT).unwrap();
        for (actual, expected) in reversed
            .control_points()
            .iter()
            .zip(curve.control_points().iter().rev())
        {
            assert_point_near(actual.point(), expected.point());
        }

        let asymmetric = NurbsCurve::try_parabola_from_focus(
            Point3::try_new(0.0, 0.0, 1.0).unwrap(),
            Point3::try_new(-1.0, 0.0, 0.25).unwrap(),
            Point3::try_new(3.0, 0.0, 2.25).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_point_near(
            asymmetric.control_points()[1].point(),
            Point3::try_new(-1.0, 0.0, 2.75).unwrap(),
        );
    }

    #[test]
    fn through_point_parabola_honors_the_opening_direction() {
        let start = Point3::try_new(-1.0, 0.0, 0.25).unwrap();
        let through = Point3::try_new(1.0, 0.0, 0.25).unwrap();
        let end = Point3::try_new(3.0, 0.0, 2.25).unwrap();
        let vertical = NurbsCurve::try_parabola_through_point(
            start,
            through,
            end,
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_point_near(
            vertical.control_points()[1].point(),
            Point3::try_new(1.0, 0.0, -0.75).unwrap(),
        );
        assert_point_near(vertical.evaluate(0.5).unwrap(), through);
        assert_point_near(
            vertical.try_parabola_focus(Tolerance::DEFAULT).unwrap(),
            Point3::try_new(0.0, 0.0, 1.0).unwrap(),
        );

        let oblique = NurbsCurve::try_parabola_through_point(
            start,
            through,
            end,
            Vector3::try_new(-1.0, 0.0, 0.75).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_point_near(
            oblique.control_points()[1].point(),
            Point3::try_new(2.904_761_904_761_904_7, 0.0, -0.178_571_428_571_428_66).unwrap(),
        );
        assert_point_near(oblique.evaluate(0.3).unwrap(), through);
    }

    #[test]
    fn three_point_parabola_rejects_degenerate_constraints() {
        let origin = Point3::try_new(0.0, 0.0, 0.0).unwrap();
        let x = Point3::try_new(1.0, 0.0, 0.0).unwrap();
        let negative_x = Point3::try_new(-1.0, 0.0, 0.0).unwrap();
        assert!(
            NurbsCurve::try_parabola_from_vertex(origin, origin, x, Tolerance::DEFAULT).is_err()
        );
        assert!(
            NurbsCurve::try_parabola_from_vertex(origin, negative_x, x, Tolerance::DEFAULT)
                .is_err()
        );
        assert!(
            NurbsCurve::try_parabola_from_focus(origin, negative_x, x, Tolerance::DEFAULT).is_err()
        );
        assert!(
            NurbsCurve::try_parabola_through_point(
                origin,
                x,
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
                Tolerance::DEFAULT,
            )
            .is_err()
        );
        assert!(
            NurbsCurve::try_parabola_through_point(
                origin,
                Point3::try_new(1.0, 1.0, -1.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
                Tolerance::DEFAULT,
            )
            .is_err()
        );
        assert!(
            NurbsCurve::try_parabola_through_point(
                origin,
                Point3::try_new(3.0, 0.0, -1.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
                Tolerance::DEFAULT,
            )
            .is_err()
        );
    }

    #[test]
    fn hyperbola_matches_rhino_rational_quadratic_layout() {
        let frame = Frame3::try_from_directions(
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let curve = NurbsCurve::try_hyperbola(frame, 3.0, 4.0, 3.75).unwrap();
        assert_eq!(curve.degree(), 2);
        assert!(curve.is_rational());
        assert_eq!(curve.domain(), 0.0..=1.0);
        assert_eq!(curve.knots(), &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        assert_eq!(
            curve.control_points(),
            &[
                WeightedPoint3::try_new(Point3::try_new(3.75, -3.0, 0.0).unwrap(), 1.0).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(2.4, 0.0, 0.0).unwrap(), 1.25).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(3.75, 3.0, 0.0).unwrap(), 1.0).unwrap(),
            ]
        );
        assert_point_near(
            curve.evaluate(0.5).unwrap(),
            Point3::try_new(3.0, 0.0, 0.0).unwrap(),
        );

        for parameter in [0.0, 0.125, 0.5, 0.875, 1.0] {
            let point = curve.evaluate(parameter).unwrap();
            let equation = point.x() * point.x() / 9.0 - point.y() * point.y() / 16.0;
            assert!((equation - 1.0).abs() < 1.0e-12);
        }
    }

    #[test]
    fn hyperbola_supports_arbitrary_frames_and_rejects_bad_dimensions() {
        let frame = Frame3::try_from_directions(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(3.0, 4.0, 0.0).unwrap(),
            Vector3::try_new(-4.0, 3.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let curve = NurbsCurve::try_hyperbola(frame, 3.0, 4.0, 3.75).unwrap();
        assert_point_near(
            curve.control_points()[0].point(),
            Point3::try_new(5.65, 3.2, 3.0).unwrap(),
        );
        assert_point_near(
            curve.control_points()[1].point(),
            Point3::try_new(2.44, 3.92, 3.0).unwrap(),
        );
        assert_point_near(
            curve.control_points()[2].point(),
            Point3::try_new(0.85, 6.8, 3.0).unwrap(),
        );

        for dimensions in [
            [0.0, 4.0, 5.0],
            [3.0, 0.0, 5.0],
            [3.0, 4.0, 3.0],
            [3.0, 4.0, 2.0],
            [3.0, 4.0, Real::INFINITY],
        ] {
            assert!(
                NurbsCurve::try_hyperbola(frame, dimensions[0], dimensions[1], dimensions[2])
                    .is_err()
            );
        }
    }

    #[test]
    fn control_point_curve_matches_rhino_degree_lowering_and_domain() {
        let controls = vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 0.5).unwrap(),
            Point3::try_new(4.0, -1.0, 2.0).unwrap(),
            Point3::try_new(4.5, 3.0, -0.5).unwrap(),
            Point3::try_new(10.0, 0.0, 1.0).unwrap(),
        ];
        let curve = NurbsCurve::try_control_point_curve(3, controls.clone()).unwrap();
        let domain_end = 17.976_753_701_093_052;
        assert_eq!(curve.degree(), 3);
        assert_eq!(
            curve
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            controls
        );
        for (actual, expected) in curve.knots().iter().zip([
            0.0,
            0.0,
            0.0,
            0.0,
            domain_end / 2.0,
            domain_end,
            domain_end,
            domain_end,
            domain_end,
        ]) {
            assert!((*actual - expected).abs() <= 2.0e-14);
        }

        let quadratic = NurbsCurve::try_control_point_curve(
            5,
            vec![point(0.0, 0.0), point(2.0, 3.0), point(10.0, 0.0)],
        )
        .unwrap();
        assert_eq!(quadratic.degree(), 2);
        assert_eq!(quadratic.domain(), 0.0..=13.0_f64.sqrt() + 73.0_f64.sqrt());

        assert!(matches!(
            NurbsCurve::try_control_point_curve(3, vec![point(1.0, 1.0), point(1.0, 1.0)]),
            Err(GeometryError::Degenerate {
                context: "control-point curve"
            })
        ));
    }

    #[test]
    fn smooth_control_point_curve_matches_rhino_periodic_layout() {
        let unique = vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 0.5).unwrap(),
            Point3::try_new(4.0, -1.0, 2.0).unwrap(),
            Point3::try_new(4.5, 3.0, -0.5).unwrap(),
            Point3::try_new(10.0, 0.0, 1.0).unwrap(),
            Point3::try_new(8.0, -4.0, 2.0).unwrap(),
            Point3::try_new(2.0, -3.0, -1.0).unwrap(),
            Point3::try_new(-2.0, 1.0, 0.25).unwrap(),
        ];
        let curve = NurbsCurve::try_control_point_curve_with_closure(
            4,
            unique.clone(),
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        assert_eq!(curve.degree(), 4);
        assert!(curve.is_periodic());
        assert!(curve.is_closed().unwrap());
        let expected_controls = unique[7..]
            .iter()
            .chain(&unique)
            .chain(&unique[..3])
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            curve
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            expected_controls
        );
        assert!((*curve.domain().end() - 46.426_262_339_780_31).abs() <= 2.0e-14);
        let short_knots = &curve.knots()[1..curve.knots().len() - 1];
        for (actual, expected) in short_knots.iter().zip([
            -17.409_848_377_417_617,
            -11.606_565_584_945_077,
            -5.803_282_792_472_538_5,
            0.0,
            5.803_282_792_472_538_5,
            11.606_565_584_945_077,
            17.409_848_377_417_617,
            23.213_131_169_890_154,
            29.016_413_962_362_69,
            34.819_696_754_835_235,
            40.622_979_547_307_77,
            46.426_262_339_780_31,
            52.229_545_132_252_845,
            58.032_827_924_725_38,
            63.836_110_717_197_926,
        ]) {
            assert!((*actual - expected).abs() <= 3.0e-14);
        }

        let lowered = NurbsCurve::try_control_point_curve_with_closure(
            11,
            unique.clone(),
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        assert_eq!(lowered.degree(), 8);
        assert_eq!(lowered.control_points().len(), 16);
        assert_eq!(lowered.control_points()[0].point(), unique[5]);
        assert!(lowered.is_periodic());
        assert!((*lowered.domain().end() - 70.187_373_289_648_95).abs() <= 3.0e-14);
    }

    #[test]
    fn closed_control_point_curves_match_rhino_degree_lowering_and_seams() {
        let points = vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 3.0, 1.0).unwrap(),
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        ];
        let smooth = NurbsCurve::try_control_point_curve_with_closure(
            5,
            points.clone(),
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        assert_eq!(smooth.degree(), 3);
        assert!(smooth.is_periodic());
        assert_eq!(
            smooth
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            vec![
                points[2], points[0], points[1], points[2], points[0], points[1]
            ]
        );
        assert!((*smooth.domain().end() - 36.085_640_040_590_51).abs() <= 2.0e-14);

        let sharp = NurbsCurve::try_control_point_curve_with_closure(
            5,
            points.clone(),
            ControlPointCurveClosure::Sharp,
        )
        .unwrap();
        assert_eq!(sharp.degree(), 3);
        assert!(!sharp.is_periodic());
        assert!(sharp.is_closed().unwrap());
        assert_eq!(
            sharp
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            vec![points[0], points[1], points[2], points[0]]
        );
        assert!((*sharp.domain().end() - 22.343_982_653_816_568).abs() <= 2.0e-14);

        let linear = NurbsCurve::try_control_point_curve_with_closure(
            1,
            points.clone(),
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        assert_eq!(linear.degree(), 1);
        assert!(!linear.is_periodic());
        assert!(linear.is_closed().unwrap());

        let mut explicitly_closed = points.clone();
        explicitly_closed.push(points[0]);
        let normalized = NurbsCurve::try_control_point_curve_with_closure(
            3,
            explicitly_closed,
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        assert_eq!(normalized, smooth);

        assert_eq!(
            NurbsCurve::try_control_point_curve_with_closure(
                3,
                points[..2].to_vec(),
                ControlPointCurveClosure::Sharp,
            ),
            Err(GeometryError::InsufficientClosedControlPoints { actual: 2 })
        );
    }

    #[test]
    fn control_polygon_uses_rhino_periodic_window_and_rejects_zero_segments() {
        let points = vec![
            point(0.0, 0.0),
            point(2.0, 0.0),
            point(3.0, 2.0),
            point(1.0, 4.0),
            point(-1.0, 2.0),
        ];
        let periodic = NurbsCurve::try_control_point_curve_with_closure(
            3,
            points.clone(),
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        assert_eq!(
            periodic
                .control_polygon(Tolerance::DEFAULT)
                .unwrap()
                .vertices(),
            &[
                points[0], points[1], points[2], points[3], points[4], points[0]
            ]
        );

        let degree_two = NurbsCurve::try_new(
            2,
            vec![points[0], points[1], points[2], points[0], points[1]],
            vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        )
        .unwrap();
        assert_eq!(
            degree_two
                .control_polygon(Tolerance::DEFAULT)
                .unwrap()
                .vertices(),
            &[points[1], points[2], points[0], points[1]]
        );

        let repeated = NurbsCurve::try_new(
            3,
            vec![points[0], points[0], points[2], points[3], points[4]],
            vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        assert_eq!(
            repeated.control_polygon(Tolerance::DEFAULT),
            Err(GeometryError::DegeneratePolylineSegment { segment: 0 })
        );
    }

    #[test]
    fn closed_state_matches_opennurbs_endpoint_and_size_rules() {
        let closed = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(3.0, 0.0),
                point(3.0, 2.0),
                point(0.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
        )
        .unwrap();
        assert!(closed.is_closed().unwrap());
        assert_eq!(
            closed.extract_point_locations().unwrap(),
            vec![point(0.0, 0.0), point(3.0, 0.0), point(3.0, 2.0)]
        );

        let nearly_closed = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(3.0, 0.0),
                point(3.0, 2.0),
                point(1.0e-10, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
        )
        .unwrap();
        assert!(nearly_closed.is_closed().unwrap());
        assert_eq!(
            nearly_closed.extract_point_locations().unwrap(),
            vec![
                point(0.0, 0.0),
                point(3.0, 0.0),
                point(3.0, 2.0),
                point(1.0e-10, 0.0),
            ]
        );

        let open = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(3.0, 0.0),
                point(3.0, 2.0),
                point(1.0e-6, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
        )
        .unwrap();
        assert!(!open.is_closed().unwrap());

        let two_segment_loop = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(3.0, 0.0), point(0.0, 0.0)],
            vec![0.0, 0.0, 1.0, 2.0, 2.0],
        )
        .unwrap();
        assert!(!two_segment_loop.is_closed().unwrap());

        let periodic = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(2.0, 0.0),
                point(1.0, 2.0),
                point(0.0, 0.0),
                point(2.0, 0.0),
            ],
            vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        )
        .unwrap();
        assert!(periodic.is_periodic());
        assert!(periodic.is_closed().unwrap());
        assert_eq!(
            periodic.extract_point_locations().unwrap(),
            vec![point(0.0, 0.0), point(2.0, 0.0), point(1.0, 2.0)]
        );
    }

    #[test]
    fn quadratic_bezier_matches_bernstein_result() {
        let curve = NurbsCurve::try_new(
            2,
            vec![point(0.0, 0.0), point(1.0, 2.0), point(2.0, 0.0)],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        assert_point_near(curve.evaluate(0.5).unwrap(), point(1.0, 1.0));
        assert_eq!(
            curve.derivative_at(0.5).unwrap(),
            Vector3::try_new(2.0, 0.0, 0.0).unwrap()
        );
    }

    #[test]
    fn exact_second_derivative_matches_polynomial_and_rational_curves() {
        let polynomial = NurbsCurve::try_new(
            2,
            vec![point(0.0, 0.0), point(1.0, 2.0), point(3.0, 1.0)],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let (_, first, second) = polynomial.evaluate_with_second_derivative(0.25).unwrap();
        assert_eq!(first, Vector3::try_new(2.5, 2.5, 0.0).unwrap());
        assert_eq!(second, Vector3::try_new(2.0, -6.0, 0.0).unwrap());

        let middle_weight = 0.5_f64.sqrt();
        let rational = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 1.0), middle_weight).unwrap(),
                WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let (_, _, exact) = rational.evaluate_with_second_derivative(0.5).unwrap();
        let step = 1.0e-5;
        let before = rational.derivative_at(0.5 - step).unwrap();
        let after = rational.derivative_at(0.5 + step).unwrap();
        let finite_difference = Vector3::try_new(
            (after.x() - before.x()) / (2.0 * step),
            (after.y() - before.y()) / (2.0 * step),
            (after.z() - before.z()) / (2.0 * step),
        )
        .unwrap();
        for (actual, expected) in exact
            .to_array()
            .into_iter()
            .zip(finite_difference.to_array())
        {
            assert!((actual - expected).abs() < 2.0e-8);
        }
    }

    #[test]
    fn reversal_negates_the_full_domain_and_parameter_direction() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(1.0, 3.0),
                point(4.0, -1.0),
                point(7.0, 2.0),
            ],
            vec![-2.0, -2.0, -2.0, 1.0, 5.0, 5.0, 5.0],
        )
        .unwrap();
        let reversed = curve.reversed().unwrap();
        assert_eq!(reversed.domain(), -5.0..=2.0);
        assert_eq!(reversed.knots(), &[-5.0, -5.0, -5.0, -1.0, 2.0, 2.0, 2.0]);
        for sample in 0..=16 {
            let normalized = sample as Real / 16.0;
            let reversed_parameter = reversed.parameter_at(normalized).unwrap();
            let original_parameter = curve.parameter_at(1.0 - normalized).unwrap();
            assert_point_near(
                reversed.evaluate(reversed_parameter).unwrap(),
                curve.evaluate(original_parameter).unwrap(),
            );
            let actual = reversed.derivative_at(reversed_parameter).unwrap();
            let expected = curve.derivative_at(original_parameter).unwrap();
            assert!(Tolerance::DEFAULT.approx_eq(actual.x(), -expected.x()));
            assert!(Tolerance::DEFAULT.approx_eq(actual.y(), -expected.y()));
            assert!(Tolerance::DEFAULT.approx_eq(actual.z(), -expected.z()));
        }
        assert_eq!(reversed.reversed().unwrap(), curve);
    }

    #[test]
    fn reparameterization_preserves_shape_and_maps_the_full_knot_vector() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                point(-2.0, 1.0),
                point(0.0, 3.0),
                point(2.0, -1.0),
                point(4.0, 2.0),
                point(7.0, 0.0),
            ],
            vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        )
        .unwrap();
        let mapped = curve.try_reparameterized(10.0..=16.0).unwrap();
        assert_eq!(mapped.domain(), 10.0..=16.0);
        assert_eq!(
            mapped.knots(),
            &[6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0]
        );
        for sample in 0..=32 {
            let normalized = sample as Real / 32.0;
            assert_point_near(
                curve
                    .evaluate(curve.parameter_at(normalized).unwrap())
                    .unwrap(),
                mapped
                    .evaluate(mapped.parameter_at(normalized).unwrap())
                    .unwrap(),
            );
        }

        for domain in [
            1.0..=1.0,
            2.0..=-1.0,
            Real::NEG_INFINITY..=1.0,
            0.0..=Real::NAN,
        ] {
            assert!(mapped.try_reparameterized(domain).is_err());
        }
    }

    #[test]
    fn rational_quadratic_represents_exact_quarter_circle() {
        let middle_weight = 0.5_f64.sqrt();
        let controls = vec![
            WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(1.0, 1.0), middle_weight).unwrap(),
            WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
        ];
        let curve =
            NurbsCurve::try_new_rational(2, controls, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
        let coordinate = 0.5_f64.sqrt();
        let midpoint = curve.evaluate(0.5).unwrap();
        assert_point_near(midpoint, point(coordinate, coordinate));
        assert!(Tolerance::DEFAULT.approx_eq(midpoint.x().hypot(midpoint.y()), 1.0));
        let tangent = curve.derivative_at(0.5).unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(tangent.x() + tangent.y(), 0.0));
        let radius = Vector3::try_new(midpoint.x(), midpoint.y(), 0.0).unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(radius.dot(tangent).unwrap(), 0.0));
        assert!(
            Tolerance::try_new(1.0e-11, 1.0e-12, 1.0e-12)
                .unwrap()
                .approx_eq(
                    curve.length(Tolerance::DEFAULT).unwrap(),
                    std::f64::consts::FRAC_PI_2
                )
        );
    }

    #[test]
    fn closest_parameter_finds_rational_arc_and_endpoint_minima() {
        let middle_weight = 0.5_f64.sqrt();
        let arc = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 1.0), middle_weight).unwrap(),
                WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let diagonal = 2.0_f64.sqrt();
        let parameter = arc
            .closest_parameter(point(diagonal, diagonal), Tolerance::DEFAULT)
            .unwrap();
        assert!((parameter - 0.5).abs() <= 1.0e-10, "parameter={parameter}");
        assert_point_near(
            arc.evaluate(parameter).unwrap(),
            point(0.5_f64.sqrt(), 0.5_f64.sqrt()),
        );

        let line = NurbsCurve::try_new(
            1,
            vec![point(-2.0, 3.0), point(4.0, 3.0)],
            vec![-5.0, -5.0, 7.0, 7.0],
        )
        .unwrap();
        assert_eq!(
            line.closest_parameter(point(-9.0, 1.0), Tolerance::DEFAULT)
                .unwrap(),
            -5.0
        );
        assert_eq!(
            line.closest_parameter(point(12.0, 5.0), Tolerance::DEFAULT)
                .unwrap(),
            7.0
        );
    }

    #[test]
    fn closest_parameter_resolves_nonuniform_multispan_lobes() {
        let curve = NurbsCurve::try_new_rational(
            3,
            vec![
                WeightedPoint3::try_new(point(-5.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(-4.0, 7.0), 0.8).unwrap(),
                WeightedPoint3::try_new(point(-1.0, -6.0), 1.7).unwrap(),
                WeightedPoint3::try_new(point(1.0, 6.0), 0.6).unwrap(),
                WeightedPoint3::try_new(point(4.0, -7.0), 1.4).unwrap(),
                WeightedPoint3::try_new(point(6.0, 1.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(9.0, 3.0), 0.9).unwrap(),
            ],
            vec![-3.0, -3.0, -3.0, -3.0, -2.4, 0.75, 2.8, 4.0, 4.0, 4.0, 4.0],
        )
        .unwrap();
        let expected_parameter = 1.83;
        let (on_curve, tangent) = curve.evaluate_with_derivative(expected_parameter).unwrap();
        let normal = Vector3::try_new(-tangent.y(), tangent.x(), 0.0)
            .unwrap()
            .normalized(Tolerance::DEFAULT)
            .unwrap();
        let target = on_curve
            .translated(normal.as_vector().scaled(0.025).unwrap())
            .unwrap();
        let actual_parameter = curve.closest_parameter(target, Tolerance::DEFAULT).unwrap();
        assert!(
            (actual_parameter - expected_parameter).abs() <= 1.0e-8,
            "actual={actual_parameter}, expected={expected_parameter}"
        );
    }

    #[test]
    fn closest_parameter_handles_large_domains_and_translations() {
        let base = 1.0e120;
        let extent = 1.0e114;
        let curve = NurbsCurve::try_new(
            2,
            vec![
                point(base - 3.0 * extent, base),
                point(base - extent, base + 4.0 * extent),
                point(base + 2.0 * extent, base - 2.0 * extent),
                point(base + 5.0 * extent, base + extent),
            ],
            vec![
                -1.0e100, -1.0e100, -1.0e100, 2.0e99, 1.0e100, 1.0e100, 1.0e100,
            ],
        )
        .unwrap();
        let expected_parameter = 6.5e99;
        let (on_curve, tangent) = curve.evaluate_with_derivative(expected_parameter).unwrap();
        let normal = Vector3::try_new(-tangent.y(), tangent.x(), 0.0)
            .unwrap()
            .normalized(Tolerance::DEFAULT)
            .unwrap();
        let target = on_curve
            .translated(normal.as_vector().scaled(1.0e112).unwrap())
            .unwrap();
        let actual_parameter = curve.closest_parameter(target, Tolerance::DEFAULT).unwrap();
        assert!(
            ((actual_parameter - expected_parameter) / expected_parameter).abs() <= 1.0e-10,
            "actual={actual_parameter:e}, expected={expected_parameter:e}"
        );
    }

    #[test]
    fn clamped_uniform_curve_has_expected_knots_and_endpoints() {
        let controls = vec![
            point(0.0, 0.0),
            point(1.0, 2.0),
            point(2.0, 2.0),
            point(3.0, 0.0),
            point(4.0, 1.0),
        ];
        let curve = NurbsCurve::try_clamped_uniform(3, controls.clone()).unwrap();
        assert_eq!(
            curve.knots(),
            &[0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]
        );
        assert_eq!(curve.evaluate(0.0).unwrap(), controls[0]);
        assert_eq!(curve.evaluate(1.0).unwrap(), controls[4]);
    }

    #[test]
    fn rejects_structural_errors_and_out_of_domain_parameters() {
        let controls = vec![point(0.0, 0.0), point(1.0, 1.0), point(2.0, 0.0)];
        assert!(NurbsCurve::try_new(0, controls.clone(), vec![]).is_err());
        assert!(NurbsCurve::try_new(2, controls.clone(), vec![0.0; 5]).is_err());
        assert!(
            NurbsCurve::try_new(2, controls.clone(), vec![0.0, 0.0, 0.5, 0.4, 1.0, 1.0]).is_err()
        );

        let curve = NurbsCurve::try_clamped_uniform(2, controls).unwrap();
        assert!(matches!(
            curve.evaluate(-0.1),
            Err(GeometryError::ParameterOutOfDomain { .. })
        ));
        assert!(curve.evaluate(Real::NAN).is_err());
    }

    #[test]
    fn uniformly_scaling_weights_does_not_change_curve() {
        let points = [point(0.0, 0.0), point(1.0, 2.0), point(2.0, 0.0)];
        let make_curve = |scale: Real| {
            NurbsCurve::try_new_rational(
                2,
                points
                    .into_iter()
                    .zip([1.0, 0.25, 2.0])
                    .map(|(point, weight)| WeightedPoint3::try_new(point, weight * scale).unwrap())
                    .collect(),
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            )
            .unwrap()
        };
        let ordinary = make_curve(1.0);
        let huge = make_curve(1.0e200);
        assert_point_near(
            ordinary.evaluate(0.37).unwrap(),
            huge.evaluate(0.37).unwrap(),
        );
    }

    #[test]
    fn affine_transform_preserves_knots_weights_and_evaluation() {
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(0.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 2.0), 0.5).unwrap(),
                WeightedPoint3::try_new(point(3.0, 0.0), 2.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let transform = AffineTransform3::try_new(
            [[2.0, -1.0, 0.0], [0.5, 3.0, 0.0], [0.0, 0.0, 1.0]],
            Vector3::try_new(4.0, -2.0, 7.0).unwrap(),
        )
        .unwrap();
        let transformed = curve.transformed(transform).unwrap();

        assert_eq!(transformed.knots(), curve.knots());
        assert_eq!(
            transformed
                .control_points()
                .iter()
                .map(|control| control.weight())
                .collect::<Vec<_>>(),
            vec![1.0, 0.5, 2.0]
        );
        assert_point_near(
            transformed.evaluate(0.37).unwrap(),
            transform
                .transform_point(curve.evaluate(0.37).unwrap())
                .unwrap(),
        );
    }

    #[test]
    fn evaluates_across_a_domain_whose_difference_overflows() {
        let curve = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(10.0, 0.0)],
            vec![-Real::MAX, -Real::MAX, Real::MAX, Real::MAX],
        )
        .unwrap();
        assert_eq!(curve.parameter_at(0.5).unwrap(), 0.0);
        assert_point_near(curve.evaluate(0.0).unwrap(), point(5.0, 0.0));
        let derivative = curve.derivative_at(0.0).unwrap();
        assert!(derivative.x().is_finite());
        assert!(derivative.x() > 0.0);
    }

    #[test]
    fn fully_multiple_knot_selects_right_hand_piece() {
        let curve = NurbsCurve::try_new(
            1,
            vec![
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(10.0, 0.0),
                point(11.0, 0.0),
            ],
            vec![0.0, 0.0, 0.5, 0.5, 1.0, 1.0],
        )
        .unwrap();
        assert_eq!(curve.evaluate(0.5).unwrap(), point(10.0, 0.0));
        assert!(curve.evaluate(0.5_f64.next_down()).unwrap().x() < 1.0);
        assert_eq!(curve.derivative_at(0.5).unwrap().x(), 2.0);
    }

    #[test]
    fn knot_insertion_preserves_a_rational_nonuniform_curve() {
        let curve = NurbsCurve::try_new_rational(
            3,
            vec![
                WeightedPoint3::try_new(point(-3.0, 1.0), 0.75).unwrap(),
                WeightedPoint3::try_new(point(-1.0, 4.0), 2.0).unwrap(),
                WeightedPoint3::try_new(point(2.0, -2.0), 0.4).unwrap(),
                WeightedPoint3::try_new(point(4.0, 3.0), 3.5).unwrap(),
                WeightedPoint3::try_new(point(7.0, -1.0), 1.25).unwrap(),
                WeightedPoint3::try_new(point(9.0, 2.0), 0.9).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 0.35, 0.8, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let refined = curve.try_insert_knot(0.52, 3).unwrap();

        assert_eq!(refined.degree(), curve.degree());
        assert_eq!(refined.domain(), curve.domain());
        assert_eq!(refined.knot_multiplicity(0.52).unwrap(), 3);
        assert_eq!(
            refined.control_points().len(),
            curve.control_points().len() + 3
        );
        for sample in 0..=64 {
            let parameter = sample as Real / 64.0;
            assert_point_near(
                refined.evaluate(parameter).unwrap(),
                curve.evaluate(parameter).unwrap(),
            );
            let actual = refined.derivative_at(parameter).unwrap();
            let expected = curve.derivative_at(parameter).unwrap();
            assert!(Tolerance::DEFAULT.approx_eq(actual.x(), expected.x()));
            assert!(Tolerance::DEFAULT.approx_eq(actual.y(), expected.y()));
            assert!(Tolerance::DEFAULT.approx_eq(actual.z(), expected.z()));
        }

        let fully_refined = refined.try_insert_knot(0.52, 4).unwrap();
        assert_eq!(fully_refined.knot_multiplicity(0.52).unwrap(), 4);
        for parameter in [0.0, 0.2, 0.52, 0.7, 1.0] {
            assert_point_near(
                fully_refined.evaluate(parameter).unwrap(),
                curve.evaluate(parameter).unwrap(),
            );
        }
        let actual = fully_refined.derivative_at(0.52).unwrap();
        let expected = curve.derivative_at(0.52).unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(actual.x(), expected.x()));
        assert!(Tolerance::DEFAULT.approx_eq(actual.y(), expected.y()));
        assert!(Tolerance::DEFAULT.approx_eq(actual.z(), expected.z()));
        assert_eq!(
            fully_refined.try_insert_knot(0.52, 2).unwrap(),
            fully_refined
        );
    }

    #[test]
    fn endpoint_refinement_preserves_nonclamped_curve_evaluation() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                point(-2.0, 1.0),
                point(0.0, 4.0),
                point(5.0, -2.0),
                point(8.0, 3.0),
            ],
            vec![-2.0, -1.0, 0.0, 0.8, 2.0, 3.0, 4.0],
        )
        .unwrap();

        for parameter in [0.0, 2.0] {
            let refined = curve.try_insert_knot(parameter, 3).unwrap();
            assert_eq!(refined.knot_multiplicity(parameter).unwrap(), 3);
            for sample in 0..=32 {
                let t = sample as Real / 16.0;
                let actual = refined.evaluate(t).unwrap();
                let expected = curve.evaluate(t).unwrap();
                assert!(
                    actual.is_near(
                        expected,
                        Tolerance::try_new(1.0e-12, 1.0e-12, 1.0e-12).unwrap()
                    ),
                    "refined endpoint {parameter}, sample {t}: actual {actual:?}, expected {expected:?}"
                );
            }
        }
    }

    #[test]
    fn split_preserves_rational_curve_parameters_and_derivatives() {
        let middle_weight = 0.5_f64.sqrt();
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 1.0), middle_weight).unwrap(),
                WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let (left, right) = curve.try_split(0.4).unwrap();

        assert_eq!(left.domain(), 0.0..=0.4);
        assert_eq!(right.domain(), 0.4..=1.0);
        assert!(
            left.knots()[left.knots().len() - 3..]
                .iter()
                .all(|knot| *knot == 0.4)
        );
        assert!(right.knots()[..3].iter().all(|knot| *knot == 0.4));
        assert_point_near(left.evaluate(0.4).unwrap(), right.evaluate(0.4).unwrap());
        for (piece, start, end) in [(&left, 0.0_f64, 0.4_f64), (&right, 0.4, 1.0)] {
            for sample in 0..=16 {
                let fraction = sample as Real / 16.0;
                let parameter = start.mul_add(1.0 - fraction, end * fraction);
                assert_point_near(
                    piece.evaluate(parameter).unwrap(),
                    curve.evaluate(parameter).unwrap(),
                );
                let actual = piece.derivative_at(parameter).unwrap();
                let expected = curve.derivative_at(parameter).unwrap();
                assert!(Tolerance::DEFAULT.approx_eq(actual.x(), expected.x()));
                assert!(Tolerance::DEFAULT.approx_eq(actual.y(), expected.y()));
                assert!(Tolerance::DEFAULT.approx_eq(actual.z(), expected.z()));
            }
        }
    }

    #[test]
    fn split_reuses_degree_multiplicity_control_and_preserves_full_break() {
        let continuous = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(1.0, 3.0),
                point(2.0, 1.0),
                point(5.0, -2.0),
                point(8.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let (left, right) = continuous.try_split(0.5).unwrap();
        assert_eq!(left.control_points().len(), 3);
        assert_eq!(right.control_points().len(), 3);
        assert_eq!(left.control_points()[2], right.control_points()[0]);
        assert_point_near(
            left.evaluate(0.5).unwrap(),
            continuous.evaluate(0.5).unwrap(),
        );
        assert_point_near(
            right.evaluate(0.5).unwrap(),
            continuous.evaluate(0.5).unwrap(),
        );

        let discontinuous = NurbsCurve::try_new(
            1,
            vec![
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(10.0, 0.0),
                point(11.0, 0.0),
            ],
            vec![0.0, 0.0, 0.5, 0.5, 1.0, 1.0],
        )
        .unwrap();
        let (left, right) = discontinuous.try_split(0.5).unwrap();
        assert_eq!(left.control_points().len(), 2);
        assert_eq!(right.control_points().len(), 2);
        assert_eq!(left.evaluate(0.5).unwrap(), point(1.0, 0.0));
        assert_eq!(right.evaluate(0.5).unwrap(), point(10.0, 0.0));
        assert_ne!(left.control_points()[1], right.control_points()[0]);
    }

    #[test]
    fn partial_trim_is_exact_and_clamps_nonclamped_ends() {
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(-2.0, 1.0), 0.75).unwrap(),
                WeightedPoint3::try_new(point(0.0, 4.0), 2.0).unwrap(),
                WeightedPoint3::try_new(point(5.0, -2.0), 0.5).unwrap(),
                WeightedPoint3::try_new(point(8.0, 3.0), 1.25).unwrap(),
            ],
            vec![-2.0, -1.0, 0.0, 0.8, 2.0, 3.0, 4.0],
        )
        .unwrap();
        let trimmed = curve.try_trimmed(0.25..=1.6).unwrap();

        assert_eq!(trimmed.domain(), 0.25..=1.6);
        assert!(trimmed.knots()[..3].iter().all(|knot| *knot == 0.25));
        assert!(
            trimmed.knots()[trimmed.knots().len() - 3..]
                .iter()
                .all(|knot| *knot == 1.6)
        );
        for sample in 0..=32 {
            let fraction = sample as Real / 32.0;
            let parameter = 0.25_f64.mul_add(1.0 - fraction, 1.6 * fraction);
            assert_point_near(
                trimmed.evaluate(parameter).unwrap(),
                curve.evaluate(parameter).unwrap(),
            );
        }

        let from_start = curve.try_trimmed(0.0..=1.6).unwrap();
        assert!(from_start.knots()[..3].iter().all(|knot| *knot == 0.0));
        assert_point_near(
            from_start.evaluate(0.0).unwrap(),
            curve.evaluate(0.0).unwrap(),
        );
        assert_eq!(curve.try_trimmed(curve.domain()).unwrap(), curve);
    }

    #[test]
    fn splitting_periodic_curve_opens_and_clamps_both_pieces() {
        let curve = NurbsCurve::try_control_point_curve_with_closure(
            3,
            vec![
                point(-3.0, 0.0),
                point(-1.0, 3.0),
                point(2.0, 4.0),
                point(5.0, 1.0),
                point(4.0, -3.0),
                point(0.0, -4.0),
            ],
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        let domain = curve.domain();
        assert_eq!(curve.try_trimmed(domain.clone()).unwrap(), curve);
        let split = curve.parameter_at(0.43).unwrap();
        let (left, right) = curve.try_split(split).unwrap();

        assert!(!left.is_periodic());
        assert!(!right.is_periodic());
        assert!(
            left.knots()[..4]
                .iter()
                .all(|knot| *knot == *domain.start())
        );
        assert!(
            left.knots()[left.knots().len() - 4..]
                .iter()
                .all(|knot| *knot == split)
        );
        assert!(right.knots()[..4].iter().all(|knot| *knot == split));
        assert!(
            right.knots()[right.knots().len() - 4..]
                .iter()
                .all(|knot| *knot == *domain.end())
        );
        for sample in 0..=40 {
            let parameter = curve.parameter_at(sample as Real / 40.0).unwrap();
            let piece = if parameter <= split { &left } else { &right };
            assert_point_near(
                piece.evaluate(parameter).unwrap(),
                curve.evaluate(parameter).unwrap(),
            );
        }
        assert_point_near(
            left.evaluate(*domain.start()).unwrap(),
            right.evaluate(*domain.end()).unwrap(),
        );
    }

    #[test]
    fn refinement_handles_large_coordinates_and_weight_ranges() {
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(Point3::try_new(-1.0e300, 2.0e299, 0.0).unwrap(), 1.0e-100)
                    .unwrap(),
                WeightedPoint3::try_new(Point3::try_new(8.0e299, -4.0e299, 0.0).unwrap(), 1.0e100)
                    .unwrap(),
                WeightedPoint3::try_new(Point3::try_new(1.0e300, 6.0e299, 0.0).unwrap(), 2.0e99)
                    .unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let refined = curve.try_insert_knot(0.5, 2).unwrap();
        for parameter in [0.0, 0.2, 0.5, 0.8, 1.0] {
            let actual = refined.evaluate(parameter).unwrap().to_array();
            let expected = curve.evaluate(parameter).unwrap().to_array();
            assert!(
                actual
                    .into_iter()
                    .zip(expected)
                    .all(|(actual, expected)| Tolerance::DEFAULT.approx_eq(actual, expected))
            );
        }
        assert!(refined.control_points().iter().all(|control| {
            control.weight().is_finite()
                && control.point().to_array().into_iter().all(Real::is_finite)
        }));
    }

    #[test]
    fn refinement_split_and_trim_reject_invalid_requests() {
        let curve = NurbsCurve::try_clamped_uniform(
            2,
            vec![point(0.0, 0.0), point(1.0, 2.0), point(3.0, 0.0)],
        )
        .unwrap();
        assert_eq!(
            curve.try_insert_knot(0.5, 0),
            Err(GeometryError::InvalidKnotMultiplicity {
                actual: 0,
                maximum: 3
            })
        );
        assert!(matches!(
            curve.try_insert_knot(0.5, 4),
            Err(GeometryError::InvalidKnotMultiplicity { .. })
        ));
        assert!(matches!(
            curve.try_insert_knot(-0.1, 1),
            Err(GeometryError::ParameterOutOfDomain { .. })
        ));
        assert!(curve.try_insert_knot(Real::NAN, 1).is_err());
        for parameter in [-1.0, 0.0, 1.0, 2.0, Real::NAN] {
            assert!(curve.try_split(parameter).is_err());
        }
        for interval in [
            0.5..=0.5,
            0.8..=0.2,
            -0.1..=0.8,
            0.2..=1.1,
            Real::NAN..=0.5,
            0.2..=Real::INFINITY,
        ] {
            assert_eq!(
                curve.try_trimmed(interval),
                Err(GeometryError::InvalidCurveTrimInterval)
            );
        }
    }
}
