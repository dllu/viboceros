use std::ops::RangeInclusive;

use faer::{Mat, prelude::*};

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

/// A Euclidean control point paired with a finite, nonzero rational weight.
///
/// Negative weights are required for projective NURBS produced by Rhino
/// operations such as deformable degree changes. Evaluation still fails at a
/// parameter where the blended rational denominator vanishes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedPoint3 {
    point: Point3,
    weight: Real,
}

impl WeightedPoint3 {
    pub fn try_new(point: Point3, weight: Real) -> Result<Self, GeometryError> {
        if weight.is_finite() && weight != 0.0 {
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
    /// normalized parameter direction are preserved. The stored OpenNURBS
    /// short knot vector is mapped in full; the two artificial full-vector
    /// endpoints are then reconstructed from its first and last values.
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
        Ok(
            Self::try_new_rational(self.degree, self.control_points.clone(), knots)?
                .with_opennurbs_outer_knots(),
        )
    }

    /// Replaces the knot vector with Rhino-compatible unit spacing while
    /// leaving the degree, control locations, and rational weights unchanged.
    ///
    /// Clamping is detected independently at the start and end. This mirrors
    /// OpenNURBS' omitted-end-knot convention: an unclamped end retains only
    /// the duplicated artificial knot in our full representation, while a
    /// clamped end retains its full multiplicity. The curve's shape and
    /// parameter domain can therefore change.
    pub fn try_make_uniform(&self) -> Result<Self, GeometryError> {
        let knots = uniform_knots_like(self.degree, self.control_points.len(), &self.knots)?;
        Self::try_new_rational(self.degree, self.control_points.clone(), knots)
    }

    /// Changes the polynomial degree using Rhino's knot-structure rules.
    ///
    /// Raising with `deformable = false` preserves the exact homogeneous
    /// curve and parameterization by increasing every knot multiplicity by the
    /// degree delta. Lowering, or either direction with `deformable = true`,
    /// retains each distinct active knot break once and interpolates the
    /// source at the target Greville abscissae. A periodic source is clamped
    /// when its degree changes, matching OpenNURBS degree elevation.
    pub fn try_change_degree(
        &self,
        desired_degree: usize,
        deformable: bool,
    ) -> Result<Self, GeometryError> {
        if desired_degree == 0 {
            return Err(GeometryError::InvalidDegree);
        }
        if desired_degree == self.degree {
            return Ok(self.clone());
        }

        let source = self.clamped_to_active_domain()?;
        let knots = changed_degree_knots(source.degree, desired_degree, deformable, &source.knots)?;
        source.interpolate_homogeneous_in_basis(
            desired_degree,
            knots,
            GeometryError::DegreeChangeSolveFailed,
        )
    }

    /// Converts a periodic curve to the equivalent clamped, non-periodic form
    /// without changing its active domain, parameterization, or locus.
    /// Curves that are already non-periodic are returned unchanged.
    pub fn try_make_non_periodic(&self) -> Result<Self, GeometryError> {
        if self.is_periodic() {
            self.clamped_to_active_domain()
        } else {
            Ok(self.clone())
        }
    }

    /// Converts a closed degree-two-or-higher curve to Rhino-compatible
    /// periodic form.
    ///
    /// With `smooth = false`, existing homogeneous controls are cyclically
    /// retained and only the seam knot distribution changes. With
    /// `smooth = true`, the active knot breaks are retained and a periodic
    /// interpolation solve at their Greville abscissae smooths the seam.
    pub fn try_make_periodic(&self, smooth: bool) -> Result<Self, GeometryError> {
        if self.is_periodic() {
            return Ok(self.clone());
        }
        if self.degree < 2 {
            return Err(GeometryError::PeriodicNurbsDegreeTooLow);
        }
        if !self.is_closed()? {
            return Err(GeometryError::PeriodicCurveMustBeClosed);
        }
        self.try_make_periodic_assuming_closed(smooth)
    }

    pub(crate) fn try_make_periodic_assuming_closed(
        &self,
        smooth: bool,
    ) -> Result<Self, GeometryError> {
        if self.is_periodic() {
            return Ok(self.clone());
        }
        if self.degree < 2 {
            return Err(GeometryError::PeriodicNurbsDegreeTooLow);
        }
        if smooth {
            self.make_periodic_smooth()
        } else {
            self.make_periodic_with_minimal_control_change()
        }
    }

    fn make_periodic_with_minimal_control_change(&self) -> Result<Self, GeometryError> {
        let degree = self.degree;
        let source_count = self.control_points.len();
        let mut unique_controls = if degree.is_multiple_of(2) {
            self.control_points.clone()
        } else {
            self.control_points[..source_count - 1].to_vec()
        };
        unique_controls.rotate_right(degree / 2);
        let output_count =
            unique_controls
                .len()
                .checked_add(degree)
                .ok_or(GeometryError::InvalidKnotVector {
                    context: "periodic curve control-point count overflowed usize",
                })?;
        let mut controls = Vec::new();
        controls
            .try_reserve_exact(output_count)
            .map_err(|_| GeometryError::InvalidKnotVector {
                context: "periodic curve controls exceed addressable memory",
            })?;
        controls.extend_from_slice(&unique_controls);
        controls.extend_from_slice(&unique_controls[..degree]);

        let knots = minimal_change_periodic_knots(degree, source_count, output_count, &self.knots)?;
        Self::try_new_rational(degree, controls, knots)
    }

    fn make_periodic_smooth(&self) -> Result<Self, GeometryError> {
        let degree = self.degree;
        let control_count = self.control_points.len();
        let required = if degree == 2 {
            5
        } else {
            degree
                .checked_mul(2)
                .ok_or(GeometryError::PeriodicNurbsDegreeTooLow)?
        };
        if control_count < required {
            return Err(GeometryError::InsufficientSmoothPeriodicControlPoints {
                degree,
                required,
                actual: control_count,
            });
        }
        let unique_count = control_count - degree;
        let knots = periodic_knots_preserving_active(degree, control_count, &self.knots)?;
        let parameters = periodic_greville_parameters(degree, control_count, unique_count, &knots)?;
        let weight_scale = self
            .control_points
            .iter()
            .map(|control| control.weight.abs())
            .fold(0.0, Real::max);
        let mut rows = Vec::new();
        let mut targets = Vec::new();
        rows.try_reserve_exact(unique_count)
            .map_err(|_| GeometryError::InvalidKnotVector {
                context: "periodic interpolation rows exceed addressable memory",
            })?;
        targets
            .try_reserve_exact(unique_count)
            .map_err(|_| GeometryError::InvalidKnotVector {
                context: "periodic interpolation targets exceed addressable memory",
            })?;
        for parameter in parameters {
            let basis = bspline_basis_values(&knots, degree, control_count, parameter)?;
            let mut folded = vec![0.0; unique_count];
            for (index, value) in basis.into_iter().enumerate() {
                folded[index % unique_count] += value;
            }
            rows.push(folded);
            targets.push(self.evaluate_scaled_homogeneous(parameter, weight_scale)?);
        }

        let matrix = Mat::from_fn(unique_count, unique_count, |row, column| rows[row][column]);
        let right_hand_side = Mat::from_fn(unique_count, 4, |row, column| targets[row][column]);
        let solution = matrix.full_piv_lu().solve(&right_hand_side);
        let mut unique_controls = Vec::new();
        unique_controls
            .try_reserve_exact(unique_count)
            .map_err(|_| GeometryError::InvalidKnotVector {
                context: "periodic solution controls exceed addressable memory",
            })?;
        for row in 0..unique_count {
            let normalized_weight = solution[(row, 3)];
            let weight = normalized_weight * weight_scale;
            let coordinates = [
                solution[(row, 0)] / normalized_weight,
                solution[(row, 1)] / normalized_weight,
                solution[(row, 2)] / normalized_weight,
            ];
            require_finite(
                coordinates.into_iter().chain([normalized_weight, weight]),
                "smooth periodic NURBS controls",
            )?;
            if normalized_weight == 0.0 || weight == 0.0 {
                return Err(GeometryError::PeriodicInterpolationSolveFailed);
            }
            unique_controls.push(WeightedPoint3::try_new(
                Point3::try_from(coordinates)?,
                weight,
            )?);
        }

        let mut controls = Vec::new();
        controls.try_reserve_exact(control_count).map_err(|_| {
            GeometryError::InvalidKnotVector {
                context: "periodic curve controls exceed addressable memory",
            }
        })?;
        controls.extend_from_slice(&unique_controls);
        controls.extend_from_slice(&unique_controls[..degree]);
        Self::try_new_rational(degree, controls, knots)
    }

    fn evaluate_scaled_homogeneous(
        &self,
        parameter: Real,
        weight_scale: Real,
    ) -> Result<[Real; 4], GeometryError> {
        let span = self.checked_span(parameter)?;
        let first = span - self.degree;
        let controls = self.control_points[first..=span]
            .iter()
            .map(|control| {
                let weight = control.weight / weight_scale;
                let point = control.point;
                let value = [
                    point.x() * weight,
                    point.y() * weight,
                    point.z() * weight,
                    weight,
                ];
                require_finite(value, "smooth periodic homogeneous controls")?;
                Ok(value)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        de_boor(&self.knots, self.degree, span, parameter, controls)
    }

    fn interpolate_homogeneous_in_basis(
        &self,
        degree: usize,
        knots: Vec<Real>,
        solve_failure: GeometryError,
    ) -> Result<Self, GeometryError> {
        let control_count = knots
            .len()
            .checked_sub(degree)
            .and_then(|count| count.checked_sub(1))
            .ok_or(GeometryError::InvalidKnotVector {
                context: "basis-interpolation knot vector is too short",
            })?;
        let weight_scale = self
            .control_points
            .iter()
            .map(|control| control.weight.abs())
            .fold(0.0, Real::max);
        let mut rows = Vec::new();
        let mut targets = Vec::new();
        rows.try_reserve_exact(control_count)
            .map_err(|_| GeometryError::InvalidKnotVector {
                context: "basis-interpolation rows exceed addressable memory",
            })?;
        targets
            .try_reserve_exact(control_count)
            .map_err(|_| GeometryError::InvalidKnotVector {
                context: "basis-interpolation targets exceed addressable memory",
            })?;
        for control in 0..control_count {
            let parameter = stable_knot_mean(&knots[control + 1..control + degree + 1])?;
            rows.push(bspline_basis_values(
                &knots,
                degree,
                control_count,
                parameter,
            )?);
            targets.push(self.evaluate_scaled_homogeneous(parameter, weight_scale)?);
        }

        let matrix = Mat::from_fn(control_count, control_count, |row, column| {
            rows[row][column]
        });
        let right_hand_side = Mat::from_fn(control_count, 4, |row, column| targets[row][column]);
        let solution = matrix.full_piv_lu().solve(&right_hand_side);
        let mut controls = Vec::new();
        controls.try_reserve_exact(control_count).map_err(|_| {
            GeometryError::InvalidKnotVector {
                context: "basis-interpolated controls exceed addressable memory",
            }
        })?;
        for row in 0..control_count {
            let normalized_weight = solution[(row, 3)];
            let weight = normalized_weight * weight_scale;
            let coordinates = [
                solution[(row, 0)] / normalized_weight,
                solution[(row, 1)] / normalized_weight,
                solution[(row, 2)] / normalized_weight,
            ];
            require_finite(
                coordinates.into_iter().chain([normalized_weight, weight]),
                "basis-interpolated homogeneous controls",
            )?;
            if normalized_weight == 0.0 || weight == 0.0 {
                return Err(solve_failure.clone());
            }
            controls.push(WeightedPoint3::try_new(
                Point3::try_from(coordinates)?,
                weight,
            )?);
        }
        controls[0] = self.control_points[0];
        controls[control_count - 1] = self.control_points[self.control_points.len() - 1];
        Self::try_new_rational(degree, controls, knots)
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

    /// Relocates a closed curve's seam to `parameter` without changing its
    /// locus or traversal direction.
    ///
    /// The result starts at `parameter` and retains the source domain length.
    /// A smooth periodic seam remains periodic and a seam between existing
    /// knots adds one control, as Rhino does. Rhino clamps a periodic curve
    /// when the new seam is already a multiple knot. Non-periodic curves are
    /// split exactly and cyclically appended, retaining rational weights.
    pub fn try_change_closed_seam(&self, parameter: Real) -> Result<Self, GeometryError> {
        self.validate_parameter(parameter)?;
        if !self.is_closed()? {
            return Err(GeometryError::CurveSeamMustBeClosed);
        }

        self.change_closed_seam_with_periodic_topology(parameter, self.is_periodic())
    }

    /// Surface seam relocation flattens an entire control-net direction into
    /// one high-dimensional non-rational curve. Individual three-dimensional
    /// rows can be constant or have different apparent periodicity, so the
    /// surface supplies the flattened curve's already-validated topology.
    pub(crate) fn try_change_closed_seam_with_periodic_topology(
        &self,
        parameter: Real,
        periodic: bool,
    ) -> Result<Self, GeometryError> {
        self.validate_parameter(parameter)?;
        self.change_closed_seam_with_periodic_topology(parameter, periodic)
    }

    fn change_closed_seam_with_periodic_topology(
        &self,
        parameter: Real,
        periodic: bool,
    ) -> Result<Self, GeometryError> {
        let domain = self.domain();
        let start = *domain.start();
        let end = *domain.end();
        if parameter == start {
            return Ok(self.clone());
        }
        if parameter == end {
            return self.translate_parameterization_by_period(start, end, 1);
        }
        if periodic && self.knot_multiplicity_unchecked(parameter) <= 1 {
            self.change_periodic_seam(parameter, start, end)
        } else {
            self.change_non_periodic_seam(parameter, start, end, periodic)
        }
    }

    fn translate_parameterization_by_period(
        &self,
        domain_start: Real,
        domain_end: Real,
        periods: isize,
    ) -> Result<Self, GeometryError> {
        let knots = self
            .knots
            .iter()
            .map(|knot| translate_curve_parameter(*knot, domain_start, domain_end, periods))
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_new_rational(self.degree, self.control_points.clone(), knots)
    }

    fn change_periodic_seam(
        &self,
        parameter: Real,
        domain_start: Real,
        domain_end: Real,
    ) -> Result<Self, GeometryError> {
        debug_assert!(self.is_periodic());
        let refined = self.try_insert_knot(parameter, 1)?;
        let degree = refined.degree;
        let control_count = refined.control_points.len();
        let unique_count = control_count - degree;
        let active_knots = &refined.knots[degree..=control_count];
        let seam_offset = active_knots.partition_point(|knot| *knot < parameter);
        if active_knots.get(seam_offset) != Some(&parameter) || seam_offset >= unique_count {
            return Err(GeometryError::InvalidKnotVector {
                context: "the periodic seam knot could not be located",
            });
        }

        let mut controls = Vec::new();
        controls.try_reserve_exact(control_count).map_err(|_| {
            GeometryError::InvalidKnotVector {
                context: "the seam-relocated control count exceeds addressable memory",
            }
        })?;
        for offset in 0..unique_count {
            controls.push(refined.control_points[(seam_offset + offset) % unique_count]);
        }
        for offset in 0..degree {
            controls.push(controls[offset]);
        }

        // OpenNURBS stores the periodic short knot vector. Place the requested
        // seam at short index `degree - 1`, extend the cyclic knot sequence on
        // both sides, then restore our duplicated artificial outer knots.
        let short_count = control_count
            .checked_add(degree)
            .and_then(|count| count.checked_sub(1))
            .ok_or(GeometryError::InvalidKnotVector {
                context: "the seam-relocated knot count overflowed usize",
            })?;
        let seam_offset =
            isize::try_from(seam_offset).map_err(|_| GeometryError::InvalidKnotVector {
                context: "the periodic seam offset exceeds addressable memory",
            })?;
        let unique_count_signed =
            isize::try_from(unique_count).map_err(|_| GeometryError::InvalidKnotVector {
                context: "the periodic seam period exceeds addressable memory",
            })?;
        let lead = isize::try_from(degree - 1).map_err(|_| GeometryError::InvalidKnotVector {
            context: "the periodic seam degree exceeds addressable memory",
        })?;
        let mut short_knots = Vec::new();
        short_knots.try_reserve_exact(short_count).map_err(|_| {
            GeometryError::InvalidKnotVector {
                context: "the seam-relocated knot count exceeds addressable memory",
            }
        })?;
        let base_knots = &refined.knots[degree..control_count];
        for short_index in 0..short_count {
            let short_index =
                isize::try_from(short_index).map_err(|_| GeometryError::InvalidKnotVector {
                    context: "the periodic seam knot index exceeds addressable memory",
                })?;
            let cyclic_index = seam_offset
                .checked_add(short_index)
                .and_then(|index| index.checked_sub(lead))
                .ok_or(GeometryError::InvalidKnotVector {
                    context: "the periodic seam knot index overflowed addressable memory",
                })?;
            let cycle = cyclic_index.div_euclid(unique_count_signed);
            let base_index = cyclic_index.rem_euclid(unique_count_signed) as usize;
            let knot =
                translate_curve_parameter(base_knots[base_index], domain_start, domain_end, cycle)?;
            short_knots.push(knot);
        }
        short_knots[degree - 1] = parameter;
        let active_end = degree - 1 + unique_count;
        short_knots[active_end] =
            translate_curve_parameter(parameter, domain_start, domain_end, 1)?;

        let mut knots = Vec::new();
        let knot_count = short_count
            .checked_add(2)
            .ok_or(GeometryError::InvalidKnotVector {
                context: "the seam-relocated knot count overflowed usize",
            })?;
        knots
            .try_reserve_exact(knot_count)
            .map_err(|_| GeometryError::InvalidKnotVector {
                context: "the seam-relocated knot count exceeds addressable memory",
            })?;
        knots.push(short_knots[0]);
        knots.extend_from_slice(&short_knots);
        knots.push(short_knots[short_knots.len() - 1]);
        Self::try_new_rational(degree, controls, knots)
    }

    fn change_non_periodic_seam(
        &self,
        parameter: Real,
        old_start: Real,
        old_end: Real,
        refine_periodic_topology: bool,
    ) -> Result<Self, GeometryError> {
        let (left, right) =
            self.try_split_with_periodic_topology(parameter, refine_periodic_topology)?;
        let shifted_left = left.translate_parameterization_by_period(old_start, old_end, 1)?;
        let scale = right.control_points[right.control_points.len() - 1].weight
            / shifted_left.control_points[0].weight;
        require_finite([scale], "closed curve seam weight scale")?;

        let output_control_count = right
            .control_points
            .len()
            .checked_add(shifted_left.control_points.len())
            .and_then(|count| count.checked_sub(1))
            .ok_or(GeometryError::InvalidKnotVector {
                context: "the seam-relocated control count overflowed usize",
            })?;
        let mut controls = Vec::new();
        controls
            .try_reserve_exact(output_control_count)
            .map_err(|_| GeometryError::InvalidKnotVector {
                context: "the seam-relocated control count exceeds addressable memory",
            })?;
        controls.extend_from_slice(&right.control_points);
        for control in shifted_left.control_points.iter().skip(1) {
            controls.push(WeightedPoint3::try_new(
                control.point,
                control.weight * scale,
            )?);
        }

        let output_knot_count = output_control_count
            .checked_add(self.degree)
            .and_then(|count| count.checked_add(1))
            .ok_or(GeometryError::InvalidKnotVector {
                context: "the seam-relocated knot count overflowed usize",
            })?;
        let mut knots = Vec::new();
        knots.try_reserve_exact(output_knot_count).map_err(|_| {
            GeometryError::InvalidKnotVector {
                context: "the seam-relocated knot count exceeds addressable memory",
            }
        })?;
        knots.extend_from_slice(&right.knots[..right.knots.len() - 1]);
        knots.extend_from_slice(&shifted_left.knots[self.degree + 1..]);
        debug_assert_eq!(controls.len(), output_control_count);
        debug_assert_eq!(knots.len(), output_knot_count);
        Self::try_new_rational(self.degree, controls, knots)
    }

    /// Returns the exact full-knot-vector multiplicity of `parameter`.
    ///
    /// Knot equality is intentionally exact. Near knots remain distinct so a
    /// refinement never changes the caller's requested parameter value.
    pub fn knot_multiplicity(&self, parameter: Real) -> Result<usize, GeometryError> {
        self.validate_parameter(parameter)?;
        Ok(self.knots.iter().filter(|knot| **knot == parameter).count())
    }

    /// Removes the knot whose curve point is nearest the point at `parameter`
    /// and adjusts the homogeneous controls to match Rhino's result.
    ///
    /// Rhino's scripting API performs this model-space search rather than
    /// comparing parameter distances. The first knot wins equal-distance
    /// ties, and choosing an active-domain endpoint fails because endpoint
    /// knots are not removable. Periodic curves are rejected because removing
    /// one knot cannot retain their cyclic control topology. Non-clamped ends
    /// are shape-preservingly clamped before interpolation so the result keeps
    /// the source active domain and a valid knot vector.
    pub fn try_remove_knot_near(&self, parameter: Real) -> Result<Self, GeometryError> {
        self.validate_parameter(parameter)?;
        if self.is_periodic() {
            return Err(GeometryError::PeriodicKnotRemovalUnsupported { direction: "curve" });
        }
        let knot_index = self.closest_active_knot_index_by_curve_point(parameter)?;
        self.try_remove_selected_knot(parameter, knot_index)
    }

    /// Inserts one Rhino-style control point at a curve parameter.
    ///
    /// The parameter is bracketed by the source control points' Greville
    /// abscissae. The new unit-weight control is their Euclidean interpolation,
    /// and the same parameter is inserted into the knot vector. `midpoint`
    /// snaps both interpolations to the middle of that Greville interval.
    /// Periodic curves retain their unique cyclic controls and use Rhino's
    /// normalized unit-spaced periodic knots.
    pub fn try_insert_control_point(
        &self,
        parameter: Real,
        midpoint: bool,
    ) -> Result<Self, GeometryError> {
        self.validate_parameter(parameter)?;
        let (lower, upper, insertion_parameter) =
            self.control_point_insertion_interval(parameter, midpoint)?;
        let lower_control = self.control_points[lower];
        let upper_control = self.control_points[upper];
        let lower_parameter = self.control_greville_parameter(lower)?;
        let upper_parameter = self.control_greville_parameter(upper)?;
        let alpha = interval_fraction(insertion_parameter, lower_parameter, upper_parameter)?;
        let point = Point3::try_from(blend_homogeneous(
            lower_control.point.to_array(),
            upper_control.point.to_array(),
            alpha,
        )?)?;
        let control = WeightedPoint3::try_new(point, 1.0)?;

        if self.is_periodic() {
            let unique_count = self.control_points.len() - self.degree;
            let insertion_index = if upper <= unique_count {
                upper
            } else {
                upper % unique_count
            };
            let mut unique_controls = self.control_points[..unique_count].to_vec();
            unique_controls.insert(insertion_index, control);
            return Self::try_unit_periodic_from_unique_controls(self.degree, unique_controls);
        }

        let mut controls = self.control_points.clone();
        controls.insert(upper, control);
        let mut knots = self.knots.clone();
        let knot_index = knots.partition_point(|knot| *knot <= insertion_parameter);
        knots.insert(knot_index, insertion_parameter);
        Self::try_new_rational(self.degree, controls, knots)
    }

    fn control_point_insertion_interval(
        &self,
        parameter: Real,
        midpoint: bool,
    ) -> Result<(usize, usize, Real), GeometryError> {
        let control_count = self.control_points.len();
        let mut greville_parameters = Vec::new();
        greville_parameters
            .try_reserve_exact(control_count)
            .map_err(|_| GeometryError::InvalidControlNet {
                context: "control-point insertion parameters exceed addressable memory",
            })?;
        for control in 0..control_count {
            greville_parameters.push(self.control_greville_parameter(control)?);
        }

        let partition = greville_parameters.partition_point(|candidate| *candidate < parameter);
        let upper = partition.clamp(1, control_count - 1);
        let lower = upper - 1;
        let lower_parameter = greville_parameters[lower];
        let upper_parameter = greville_parameters[upper];
        if lower_parameter >= upper_parameter
            || (!midpoint
                && (parameter < lower_parameter
                    || parameter > upper_parameter
                    || (!self.is_periodic()
                        && (parameter == *self.domain().start()
                            || parameter == *self.domain().end()))))
        {
            return Err(GeometryError::NoControlPointInsertionInterval { parameter });
        }
        let insertion_parameter = if midpoint {
            stable_knot_mean(&[lower_parameter, upper_parameter])?
        } else {
            parameter
        };
        Ok((lower, upper, insertion_parameter))
    }

    fn control_greville_parameter(&self, control: usize) -> Result<Real, GeometryError> {
        stable_knot_mean(&self.knots[control + 1..control + self.degree + 1])
    }

    /// Removes one Rhino control-point grip and updates the curve structure.
    ///
    /// Open curves retain their remaining control locations and interior
    /// weights, while new rational endpoint weights are normalized. The knot
    /// associated with the removed control is dropped directly for odd
    /// degrees; for even degrees, the two central associated knots are merged
    /// at their overflow-safe mean. Removing from a single-span curve lowers
    /// its degree by one. Periodic curves remove one unique cyclic control and
    /// rebuild the repeated tail with Rhino's unit-spaced periodic knots. The
    /// periodic degree is lowered when necessary; a minimum quadratic or cubic
    /// control layout is retained because no valid periodic result can be
    /// formed.
    pub fn try_remove_control_point(&self, index: usize) -> Result<Self, GeometryError> {
        if self.is_periodic() {
            return self.try_remove_periodic_control_point(index);
        }

        let control_count = self.control_points.len();
        if index >= control_count {
            return Err(GeometryError::ControlPointIndexOutOfRange {
                direction: "curve",
                index,
                control_point_count: control_count,
            });
        }
        if control_count == 2 {
            return Err(GeometryError::InsufficientControlPoints {
                degree: 1,
                required: 2,
                actual: 1,
            });
        }

        let clamped_start = self.knots[..=self.degree]
            .iter()
            .all(|knot| *knot == self.knots[self.degree]);
        let clamped_end = self.knots[control_count..]
            .iter()
            .all(|knot| *knot == self.knots[control_count]);
        if !clamped_start || !clamped_end {
            return self
                .clamped_to_active_domain()?
                .try_remove_control_point(index);
        }

        let mut controls = self.control_points.clone();
        controls.remove(index);
        normalize_control_point_removal_end_weights(&mut controls)?;

        if controls.len() == self.degree {
            let degree = self.degree - 1;
            let knots = self.knots[1..self.knots.len() - 1].to_vec();
            if degree == 0 {
                return Err(GeometryError::InvalidDegree);
            }
            debug_assert_eq!(knots.len(), controls.len() + degree + 1);
            return Self::try_new_rational(degree, controls, knots);
        }

        let first_interior = self.degree + 1;
        let last_interior = control_count - 1;
        let half_degree_rounded_up = self.degree.div_ceil(2);
        let target = index
            .saturating_add(half_degree_rounded_up)
            .clamp(first_interior, last_interior);
        let mut knots = self.knots.clone();
        if self.degree.is_multiple_of(2) && target > first_interior && target < last_interior {
            knots[target - 1] = stable_knot_mean(&[knots[target - 1], knots[target]])?;
        }
        knots.remove(target);
        Self::try_new_rational(self.degree, controls, knots)
    }

    fn try_remove_periodic_control_point(&self, index: usize) -> Result<Self, GeometryError> {
        let unique_count = self.control_points.len() - self.degree;
        if index >= unique_count {
            return Err(GeometryError::ControlPointIndexOutOfRange {
                direction: "periodic curve",
                index,
                control_point_count: unique_count,
            });
        }
        let remaining_unique_count = unique_count - 1;
        if remaining_unique_count < 3 {
            return Self::try_unit_periodic_from_unique_controls(
                self.degree,
                self.control_points[..unique_count].to_vec(),
            );
        }

        let mut unique_controls = self.control_points[..unique_count].to_vec();
        unique_controls.remove(index);
        let degree = self.degree.min(remaining_unique_count);
        Self::try_unit_periodic_from_unique_controls(degree, unique_controls)
    }

    fn try_unit_periodic_from_unique_controls(
        degree: usize,
        unique_controls: Vec<WeightedPoint3>,
    ) -> Result<Self, GeometryError> {
        let control_count =
            unique_controls
                .len()
                .checked_add(degree)
                .ok_or(GeometryError::InvalidControlNet {
                    context: "periodic control-point edit count overflowed usize",
                })?;
        let mut controls = Vec::new();
        controls.try_reserve_exact(control_count).map_err(|_| {
            GeometryError::InvalidControlNet {
                context: "periodic control-point edit exceeds addressable memory",
            }
        })?;
        controls.extend_from_slice(&unique_controls);
        controls.extend(unique_controls.iter().copied().cycle().take(degree));
        let knot_count = control_count
            .checked_add(degree)
            .and_then(|count| count.checked_add(1))
            .ok_or(GeometryError::InvalidKnotVector {
                context: "periodic control-point edit knot count overflowed usize",
            })?;
        let knots = (0..knot_count)
            .map(|knot| knot as Real - degree as Real)
            .collect();
        Ok(Self::try_new_rational(degree, controls, knots)?.with_opennurbs_outer_knots())
    }

    /// Collapses qualifying interior multiple-knot groups in descending
    /// parameter order, matching Rhino's `RemoveMultiKnot` command.
    ///
    /// By default only multiplicities strictly between one and the degree are
    /// reduced to one. With `remove_fully_multiple_knots`, degree-multiple
    /// kinks are also eligible when their one-sided tangent angle is strictly
    /// below `maximum_kink_angle_radians`; the same strict angle test applies
    /// to the smooth groups. Degree-one knots are removed completely. Periodic
    /// curves are rejected.
    pub fn try_remove_multiple_knots(
        &self,
        remove_fully_multiple_knots: bool,
        maximum_kink_angle_radians: Real,
    ) -> Result<(Self, usize), GeometryError> {
        if !maximum_kink_angle_radians.is_finite()
            || !(0.0..=std::f64::consts::PI).contains(&maximum_kink_angle_radians)
        {
            return Err(GeometryError::InvalidKnotRemovalAngle);
        }
        if self.is_periodic() {
            return Err(GeometryError::PeriodicKnotRemovalUnsupported { direction: "curve" });
        }

        let mut removals = Vec::new();
        for (knot, multiplicity) in self.interior_knot_groups() {
            let eligible = if remove_fully_multiple_knots {
                let is_multiple = multiplicity > 1 || self.degree == 1;
                if !is_multiple || multiplicity > self.degree {
                    false
                } else {
                    let kink_angle = if multiplicity < self.degree {
                        0.0
                    } else {
                        self.kink_angle_at(knot)?
                    };
                    kink_angle < maximum_kink_angle_radians
                }
            } else {
                multiplicity > 1 && multiplicity < self.degree
            };
            if eligible {
                let removal_count = if self.degree == 1 {
                    multiplicity
                } else {
                    multiplicity - 1
                };
                removals.push((knot, removal_count));
            }
        }

        let removed = removals.iter().map(|(_, count)| *count).sum();
        if removed == 0 {
            return Ok((self.clone(), 0));
        }
        Ok((self.try_remove_multiple_knot_groups(&removals)?, removed))
    }

    pub(crate) fn try_remove_knot_near_parameter_with_periodic_topology(
        &self,
        parameter: Real,
        periodic: bool,
    ) -> Result<Self, GeometryError> {
        self.validate_parameter(parameter)?;
        if periodic {
            return Err(GeometryError::PeriodicKnotRemovalUnsupported { direction: "curve" });
        }
        let knot_index = self.nearest_active_knot_index_by_parameter(parameter)?;
        self.try_remove_selected_knot(parameter, knot_index)
    }

    fn try_remove_selected_knot(
        &self,
        parameter: Real,
        knot_index: usize,
    ) -> Result<Self, GeometryError> {
        let first_removable = self.degree + 1;
        let last_removable = self.control_points.len() - 1;
        if knot_index < first_removable || knot_index > last_removable {
            return Err(GeometryError::NoRemovableKnot { parameter });
        }

        let knot = self.knots[knot_index];
        let source = self.clamped_to_active_domain()?;
        let first_removable = source.degree + 1;
        let last_removable = source.control_points.len() - 1;
        let clamped_index = (first_removable..=last_removable)
            .find(|index| source.knots[*index] == knot)
            .ok_or(GeometryError::NoRemovableKnot { parameter })?;
        let mut knots = source.knots.clone();
        knots.remove(clamped_index);
        source.interpolate_homogeneous_in_basis(
            source.degree,
            knots,
            GeometryError::KnotRemovalSolveFailed,
        )
    }

    pub(crate) fn interior_knot_groups(&self) -> Vec<(Real, usize)> {
        let domain = self.domain();
        let mut groups = Vec::new();
        let mut index = 0;
        while index < self.knots.len() {
            let knot = self.knots[index];
            let after = self.knots.partition_point(|candidate| *candidate <= knot);
            if knot > *domain.start() && knot < *domain.end() {
                groups.push((knot, after - index));
            }
            index = after;
        }
        groups
    }

    pub(crate) fn kink_angle_at(&self, knot: Real) -> Result<Real, GeometryError> {
        let (left, right) = self.try_split(knot)?;
        let incoming = match left.derivative_at(knot)?.normalized_nonzero() {
            Ok(tangent) => tangent,
            Err(GeometryError::Degenerate { .. }) => return Ok(std::f64::consts::PI),
            Err(error) => return Err(error),
        };
        let outgoing = match right.derivative_at(knot)?.normalized_nonzero() {
            Ok(tangent) => tangent,
            Err(GeometryError::Degenerate { .. }) => return Ok(std::f64::consts::PI),
            Err(error) => return Err(error),
        };
        let cosine = incoming
            .as_vector()
            .dot(outgoing.as_vector())?
            .clamp(-1.0, 1.0);
        Ok(cosine.acos())
    }

    pub(crate) fn try_remove_multiple_knot_groups(
        &self,
        removals: &[(Real, usize)],
    ) -> Result<Self, GeometryError> {
        if removals.is_empty() {
            return Ok(self.clone());
        }
        let mut result = self.clamped_to_active_domain()?;
        for (knot, removal_count) in removals.iter().rev() {
            let first = result.knots.partition_point(|candidate| *candidate < *knot);
            let after = result
                .knots
                .partition_point(|candidate| *candidate <= *knot);
            if *removal_count == 0 || after - first < *removal_count {
                return Err(GeometryError::KnotRemovalSolveFailed);
            }
            let mut knots = result.knots.clone();
            knots.drain(after - removal_count..after);
            result = result.interpolate_homogeneous_in_basis(
                result.degree,
                knots,
                GeometryError::KnotRemovalSolveFailed,
            )?;
        }
        Ok(result)
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
        self.try_insert_knot_with_periodic_topology(
            parameter,
            target_multiplicity,
            self.is_periodic(),
        )
    }

    /// Internal refinement entry point used when a surface direction has been
    /// flattened into OpenNURBS' single high-dimensional control curve. The
    /// caller supplies that entire curve's periodic state instead of letting
    /// one three-dimensional control row decide it independently.
    pub(crate) fn try_insert_knot_with_periodic_topology(
        &self,
        parameter: Real,
        target_multiplicity: usize,
        restore_periodic_topology: bool,
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
            return Ok(self.clone().with_opennurbs_outer_knots());
        }
        let domain = self.domain();
        if parameter == *domain.start() {
            return self
                .clamped_at_start(parameter)
                .map(Self::with_opennurbs_outer_knots);
        }
        if parameter == *domain.end() {
            return self
                .clamped_at_end(parameter)
                .map(Self::with_opennurbs_outer_knots);
        }

        let periodic_span = (restore_periodic_topology && target_multiplicity <= self.degree)
            .then(|| self.find_span(parameter) - self.degree);
        let mut refined = self.clone();
        while refined.knot_multiplicity_unchecked(parameter) < target_multiplicity {
            refined = refined.insert_knot_once(parameter)?;
        }
        if let Some(span_index) = periodic_span {
            refined = refined.restore_periodic_after_knot_insertion(span_index)?;
        }
        Ok(refined.with_opennurbs_outer_knots())
    }

    /// Splits at a parameter strictly inside the active domain.
    ///
    /// Both results retain the source parameter values and are clamped at
    /// their active ends. At an existing `degree + 1` knot, the independent
    /// left- and right-hand controls remain independent.
    pub fn try_split(&self, parameter: Real) -> Result<(Self, Self), GeometryError> {
        self.try_split_with_periodic_topology(parameter, self.is_periodic())
    }

    fn try_split_with_periodic_topology(
        &self,
        parameter: Real,
        restore_periodic_topology: bool,
    ) -> Result<(Self, Self), GeometryError> {
        require_finite([parameter], "NURBS curve split parameter")?;
        let domain = self.domain();
        if parameter <= *domain.start() || parameter >= *domain.end() {
            return Err(GeometryError::InvalidCurveSplitParameter);
        }

        let multiplicity = self.knot_multiplicity_unchecked(parameter);
        let refined = if multiplicity < self.degree {
            self.try_insert_knot_with_periodic_topology(
                parameter,
                self.degree,
                restore_periodic_topology,
            )?
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

    fn closest_active_knot_index_by_curve_point(
        &self,
        parameter: Real,
    ) -> Result<usize, GeometryError> {
        let target = self.evaluate(parameter)?;
        let domain = self.domain();
        let mut closest = None;
        for knot_index in 1..self.knots.len() - 1 {
            let knot = self.knots[knot_index];
            if knot < *domain.start() || knot > *domain.end() {
                continue;
            }
            let distance = self.evaluate(knot)?.distance_to(target)?;
            if closest.is_none_or(|(_, closest_distance)| distance < closest_distance) {
                closest = Some((knot_index, distance));
            }
        }
        closest
            .map(|(knot_index, _)| knot_index)
            .ok_or(GeometryError::NoRemovableKnot { parameter })
    }

    fn nearest_active_knot_index_by_parameter(
        &self,
        parameter: Real,
    ) -> Result<usize, GeometryError> {
        let first = self.degree;
        let last = self.control_points.len();
        let active_knots = &self.knots[first..=last];
        let upper = active_knots.partition_point(|knot| *knot <= parameter);
        if upper == 0 {
            return Ok(first);
        }
        if upper == active_knots.len() {
            return Ok(last);
        }

        let lower_index = first + upper - 1;
        let upper_index = first + upper;
        let midpoint = stable_knot_mean(&[self.knots[lower_index], self.knots[upper_index]])?;
        Ok(if parameter < midpoint {
            lower_index
        } else {
            upper_index
        })
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

    /// Restores OpenNURBS' repeated controls and matching exterior knot
    /// intervals after inserting into a periodic curve. `span_index` is the
    /// zero-based active span in the pre-insertion curve.
    fn restore_periodic_after_knot_insertion(
        self,
        span_index: usize,
    ) -> Result<Self, GeometryError> {
        let degree = self.degree;
        let control_count = self.control_points.len();
        let mut controls = self.control_points;
        for leading in 0..degree {
            let trailing = control_count - degree + leading;
            if leading > span_index {
                controls[trailing] = controls[leading];
            } else {
                controls[leading] = controls[trailing];
            }
        }

        let mut knots = self.knots;
        // OpenNURBS stores the short knot vector `knots[1..len-1]`. Its
        // periodic repair copies the first degree-1 intervals to the right,
        // then the last degree-1 intervals back to the left.
        for offset in 0..degree - 1 {
            let left = degree + offset;
            let right = control_count + offset;
            knots[right + 1] = (knots[left + 1] - knots[left]) + knots[right];
        }
        for offset in 0..degree - 1 {
            let left = degree - offset;
            let right = control_count - offset;
            knots[left - 1] = (knots[right - 1] - knots[right]) + knots[left];
        }
        // These artificial full-vector endpoints are omitted by OpenNURBS.
        // Normalize them before validation so roundoff in a reparameterized
        // periodic curve cannot put the first endpoint microscopically after
        // its reconstructed neighbor (or the last before its neighbor).
        knots[0] = knots[1];
        let last = knots.len() - 1;
        knots[last] = knots[last - 1];
        Self::try_new_rational(degree, controls, knots)
    }

    fn with_opennurbs_outer_knots(mut self) -> Self {
        self.knots[0] = self.knots[1];
        let last = self.knots.len() - 1;
        self.knots[last] = self.knots[last - 1];
        self
    }

    pub(crate) fn clamped_to_active_domain(&self) -> Result<Self, GeometryError> {
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
            .map(|control_point| control_point.weight.abs())
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

fn translate_curve_parameter(
    value: Real,
    domain_start: Real,
    domain_end: Real,
    periods: isize,
) -> Result<Real, GeometryError> {
    if periods == 0 {
        return Ok(value);
    }
    if periods == 1 && value == domain_start {
        return Ok(domain_end);
    }
    if periods == -1 && value == domain_end {
        return Ok(domain_start);
    }

    let periods = periods as Real;
    let direct_period = domain_end - domain_start;
    let direct = periods.mul_add(direct_period, value);
    if direct.is_finite() && direct_period.is_finite() {
        return Ok(direct);
    }

    let scale = value.abs().max(domain_start.abs()).max(domain_end.abs());
    if !scale.is_finite() || scale == 0.0 {
        return Err(GeometryError::NonFinite {
            context: "closed curve seam parameter translation",
        });
    }
    let normalized_period = domain_end / scale - domain_start / scale;
    let translated = periods.mul_add(normalized_period, value / scale) * scale;
    require_finite([translated], "closed curve seam parameter translation")?;
    Ok(translated)
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

fn changed_degree_knots(
    source_degree: usize,
    desired_degree: usize,
    deformable: bool,
    source_knots: &[Real],
) -> Result<Vec<Real>, GeometryError> {
    debug_assert!(source_degree >= 1 && desired_degree >= 1);
    debug_assert!(source_knots.len() >= 2 * (source_degree + 1));
    let start = source_knots[source_degree];
    let end = source_knots[source_knots.len() - source_degree - 1];
    let interior = &source_knots[source_degree + 1..source_knots.len() - source_degree - 1];
    let exact_elevation = desired_degree > source_degree && !deformable;
    let degree_delta = desired_degree.saturating_sub(source_degree);
    let endpoint_multiplicity = desired_degree
        .checked_add(1)
        .ok_or(GeometryError::InvalidDegree)?;
    let mut interior_runs = Vec::new();
    let mut target_interior_count = 0_usize;
    let mut index = 0;
    while index < interior.len() {
        let value = interior[index];
        let mut next = index + 1;
        while next < interior.len() && interior[next] == value {
            next += 1;
        }
        let source_multiplicity = next - index;
        let target_multiplicity = if exact_elevation {
            source_multiplicity
                .checked_add(degree_delta)
                .ok_or(GeometryError::InvalidDegree)?
        } else {
            1
        };
        target_interior_count = target_interior_count
            .checked_add(target_multiplicity)
            .ok_or(GeometryError::InvalidKnotVector {
                context: "degree-changed interior knot count overflowed usize",
            })?;
        interior_runs.push((value, target_multiplicity));
        index = next;
    }
    let knot_count = endpoint_multiplicity
        .checked_mul(2)
        .and_then(|count| count.checked_add(target_interior_count))
        .ok_or(GeometryError::InvalidKnotVector {
            context: "degree-changed knot count overflowed usize",
        })?;
    let mut knots = Vec::new();
    knots
        .try_reserve_exact(knot_count)
        .map_err(|_| GeometryError::InvalidKnotVector {
            context: "degree-changed knots exceed addressable memory",
        })?;
    knots.extend(std::iter::repeat_n(start, endpoint_multiplicity));
    for (value, multiplicity) in interior_runs {
        knots.extend(std::iter::repeat_n(value, multiplicity));
    }
    knots.extend(std::iter::repeat_n(end, endpoint_multiplicity));
    debug_assert_eq!(knots.len(), knot_count);
    Ok(knots)
}

fn minimal_change_periodic_knots(
    degree: usize,
    source_control_count: usize,
    output_control_count: usize,
    source_knots: &[Real],
) -> Result<Vec<Real>, GeometryError> {
    let normalized_source = normalized_active_knots(degree, source_control_count, source_knots)?;
    let source_intervals = normalized_source
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect::<Vec<_>>();
    let seam_interval =
        0.5 * source_intervals[0] + 0.5 * source_intervals[source_intervals.len() - 1];
    let mut intervals = Vec::with_capacity(output_control_count - degree);
    if source_intervals.len() == 1 {
        intervals.resize(output_control_count - degree, seam_interval);
    } else {
        let seam_count = if degree.is_multiple_of(2) {
            degree / 2
        } else {
            degree.div_ceil(2)
        };
        intervals.extend(std::iter::repeat_n(seam_interval, seam_count));
        if degree.is_multiple_of(2) {
            intervals.extend_from_slice(&source_intervals);
        } else if source_intervals.len() > 2 {
            intervals.extend_from_slice(&source_intervals[1..source_intervals.len() - 1]);
        }
        intervals.extend(std::iter::repeat_n(seam_interval, seam_count));
    }
    debug_assert_eq!(intervals.len(), output_control_count - degree);

    let interval_sum = intervals.iter().sum::<Real>();
    require_finite(
        intervals.iter().copied().chain([interval_sum]),
        "minimal-change periodic knot intervals",
    )?;
    if interval_sum <= 0.0 {
        return Err(GeometryError::InvalidKnotVector {
            context: "minimal-change periodic knot intervals must have positive length",
        });
    }
    for interval in &mut intervals {
        *interval /= interval_sum;
    }
    periodic_knots_from_normalized_intervals(
        degree,
        output_control_count,
        &intervals,
        source_knots[degree],
        source_knots[source_control_count],
    )
}

fn periodic_knots_preserving_active(
    degree: usize,
    control_count: usize,
    source_knots: &[Real],
) -> Result<Vec<Real>, GeometryError> {
    let normalized_source = normalized_active_knots(degree, control_count, source_knots)?;
    let intervals = normalized_source
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect::<Vec<_>>();
    let mut knots = periodic_knots_from_normalized_intervals(
        degree,
        control_count,
        &intervals,
        source_knots[degree],
        source_knots[control_count],
    )?;
    knots[degree..=control_count].copy_from_slice(&source_knots[degree..=control_count]);
    Ok(knots)
}

fn normalized_active_knots(
    degree: usize,
    control_count: usize,
    knots: &[Real],
) -> Result<Vec<Real>, GeometryError> {
    let domain_start = knots[degree];
    let domain_end = knots[control_count];
    let mut normalized = knots[degree..=control_count]
        .iter()
        .map(|knot| reparameterize_value(*knot, domain_start, domain_end, 0.0, 1.0))
        .collect::<Result<Vec<_>, _>>()?;
    normalized[0] = 0.0;
    let last = normalized.len() - 1;
    normalized[last] = 1.0;
    Ok(normalized)
}

fn periodic_knots_from_normalized_intervals(
    degree: usize,
    control_count: usize,
    intervals: &[Real],
    domain_start: Real,
    domain_end: Real,
) -> Result<Vec<Real>, GeometryError> {
    debug_assert_eq!(intervals.len(), control_count - degree);
    let mut normalized = vec![0.0; control_count + degree + 1];
    for (offset, interval) in intervals.iter().enumerate() {
        normalized[degree + offset + 1] = normalized[degree + offset] + interval;
    }
    normalized[control_count] = 1.0;
    for offset in 0..degree - 1 {
        normalized[control_count + offset + 1] =
            normalized[control_count + offset] + intervals[offset];
    }
    for offset in 0..degree - 1 {
        normalized[degree - offset - 1] =
            normalized[degree - offset] - intervals[intervals.len() - offset - 1];
    }
    normalized[0] = normalized[1];
    let last = normalized.len() - 1;
    normalized[last] = normalized[last - 1];

    normalized
        .into_iter()
        .map(|knot| reparameterize_value(knot, 0.0, 1.0, domain_start, domain_end))
        .collect()
}

fn periodic_greville_parameters(
    degree: usize,
    control_count: usize,
    unique_count: usize,
    knots: &[Real],
) -> Result<Vec<Real>, GeometryError> {
    let domain_start = knots[degree];
    let mut start_index = None;
    for index in 0..degree {
        let value = stable_knot_mean(&knots[index + 1..index + degree + 1])?;
        if value >= domain_start {
            start_index = Some(index);
            break;
        }
    }
    let start_index = start_index.ok_or(GeometryError::PeriodicInterpolationSolveFailed)?;
    let mut parameters = (start_index..start_index + unique_count)
        .map(|index| stable_knot_mean(&knots[index + 1..index + degree + 1]))
        .collect::<Result<Vec<_>, _>>()?;
    if parameters[0] < domain_start {
        parameters[0] = domain_start;
    }
    let domain_end = knots[control_count];
    if parameters.iter().any(|parameter| {
        !parameter.is_finite() || *parameter < domain_start || *parameter > domain_end
    }) {
        return Err(GeometryError::PeriodicInterpolationSolveFailed);
    }
    Ok(parameters)
}

pub(crate) fn stable_knot_mean(knots: &[Real]) -> Result<Real, GeometryError> {
    let divisor = knots.len() as Real;
    let direct = knots.iter().sum::<Real>() / divisor;
    if direct.is_finite() {
        return Ok(direct);
    }
    let scale = knots.iter().map(|knot| knot.abs()).fold(0.0, Real::max);
    let mean = knots.iter().map(|knot| knot / scale).sum::<Real>() / divisor * scale;
    require_finite([mean], "periodic Greville abscissa")?;
    Ok(mean)
}

fn normalize_control_point_removal_end_weights(
    controls: &mut [WeightedPoint3],
) -> Result<(), GeometryError> {
    let first = controls[0];
    controls[0] = WeightedPoint3::try_new(first.point(), 1.0)?;
    let last_index = controls.len() - 1;
    let last = controls[last_index];
    controls[last_index] = WeightedPoint3::try_new(last.point(), 1.0)?;
    Ok(())
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
        .all(|(left, right)| curve_coordinates_coincident(left, right))
}

pub(crate) fn curve_coordinates_coincident(left: Real, right: Real) -> bool {
    let difference = (left - right).abs();
    difference <= CURVE_COINCIDENCE_ABSOLUTE
        || difference <= (left.abs() + right.abs()) * CURVE_COINCIDENCE_RELATIVE
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
    if !weight.is_finite() || weight == 0.0 {
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
        if !control_point.weight.is_finite() || control_point.weight == 0.0 {
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

/// Builds the unit-spaced full knot vector used by Rhino's `MakeUniform`.
/// Start and end clamping are retained independently; every other short-knot
/// interval becomes one. The first and last full knots duplicate the omitted
/// OpenNURBS endpoint values used by the oracle protocol.
pub(crate) fn uniform_knots_like(
    degree: usize,
    control_point_count: usize,
    source_knots: &[Real],
) -> Result<Vec<Real>, GeometryError> {
    validate_direction(degree, control_point_count, source_knots)?;
    let short_knot_count = source_knots.len() - 2;
    let start_clamped = source_knots[1] == source_knots[degree];
    let end_clamped = source_knots[control_point_count] == source_knots[source_knots.len() - 2];
    let start_offset = if start_clamped { degree - 1 } else { 0 };
    let end_value = end_clamped.then(|| {
        short_knot_count
            .saturating_sub(degree)
            .saturating_sub(start_offset)
    });

    let knots = (0..source_knots.len())
        .map(|full_index| {
            let short_index = full_index.saturating_sub(1).min(short_knot_count - 1);
            let value = short_index.saturating_sub(start_offset);
            value.min(end_value.unwrap_or(value)) as Real
        })
        .collect::<Vec<_>>();
    require_finite(knots.iter().copied(), "uniform NURBS knots")?;
    Ok(knots)
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

    // Work with weights normalized by their largest magnitude. This avoids the
    // overflow in `weight * coordinate` that a literal homogeneous blend can
    // encounter, while producing the identical projective control point.
    let scale = left.weight.abs().max(right.weight.abs());
    let left_weight = left.weight / scale;
    let right_weight = right.weight / scale;
    let complement = 1.0 - alpha;
    let normalized_weight = left_weight.mul_add(complement, right_weight * alpha);
    if !normalized_weight.is_finite() || normalized_weight == 0.0 {
        return Err(GeometryError::NonFinite {
            context: "knot-insertion control weight",
        });
    }
    let weight = normalized_weight * scale;
    if !weight.is_finite() || weight == 0.0 {
        return Err(GeometryError::NonFinite {
            context: "knot-insertion control weight",
        });
    }

    let right_fraction = (right_weight * alpha) / normalized_weight;
    require_finite([right_fraction], "knot-insertion projective blend factor")?;
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
    fn closed_curve_seam_relocation_rotates_periodic_structure_exactly() {
        let unique = [
            point(0.0, 0.0),
            point(2.0, 0.0),
            point(2.0, 2.0),
            point(0.0, 2.0),
        ];
        let curve = NurbsCurve::try_new(
            3,
            unique.iter().chain(&unique[..3]).copied().collect(),
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
        )
        .unwrap();
        assert!(curve.is_periodic());

        let relocated = curve.try_change_closed_seam(3.25).unwrap();
        assert!(relocated.is_periodic());
        assert_eq!(relocated.domain(), 3.25..=7.25);
        assert_eq!(relocated.control_points().len(), 8);
        assert_eq!(
            relocated.knots(),
            &[2.0, 2.0, 3.0, 3.25, 4.0, 5.0, 6.0, 7.0, 7.25, 8.0, 9.0, 9.0]
        );
        let expected_controls = [
            point(2.0, 1.5),
            point(7.0 / 6.0, 2.0),
            point(0.0, 11.0 / 6.0),
            unique[0],
            unique[1],
            point(2.0, 1.5),
            point(7.0 / 6.0, 2.0),
            point(0.0, 11.0 / 6.0),
        ];
        for (actual, expected) in relocated.control_points().iter().zip(expected_controls) {
            assert_point_near(actual.point(), expected);
        }
        for sample in 0..=40 {
            let parameter = 3.25 + sample as Real / 40.0 * 4.0;
            let source_parameter = if parameter <= 6.0 {
                parameter
            } else {
                parameter - 4.0
            };
            assert_point_near(
                relocated.evaluate(parameter).unwrap(),
                curve.evaluate(source_parameter).unwrap(),
            );
        }

        assert_eq!(curve.try_change_closed_seam(2.0).unwrap(), curve);
        let shifted = curve.try_change_closed_seam(6.0).unwrap();
        assert_eq!(shifted.domain(), 6.0..=10.0);
        assert_eq!(
            shifted.knots(),
            &[4.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 12.0]
        );
        assert_eq!(shifted.control_points(), curve.control_points());

        let reparameterized = curve.try_reparameterized(100.0..=180.0).unwrap();
        let relocated = reparameterized.try_change_closed_seam(133.0).unwrap();
        assert!(relocated.is_periodic());
        assert_eq!(relocated.domain(), 133.0..=213.0);
        assert_eq!(relocated.control_points().len(), 8);
    }

    #[test]
    fn closed_curve_seam_relocation_preserves_rational_segments_and_rhino_knot_rules() {
        let curve = NurbsCurve::try_new_rational(
            2,
            [
                ([0.0, 0.0, 0.0], 0.5),
                ([3.0, -1.0, 0.0], 1.3),
                ([5.0, 3.0, 0.0], 0.8),
                ([0.0, 4.0, 0.0], 1.7),
                ([0.0, 0.0, 0.0], 2.0),
            ]
            .into_iter()
            .map(|(coordinates, weight)| {
                WeightedPoint3::try_new(Point3::try_from(coordinates).unwrap(), weight).unwrap()
            })
            .collect(),
            vec![4.0, 4.0, 4.0, 5.0, 9.0, 12.0, 12.0, 12.0],
        )
        .unwrap();
        assert!(curve.is_closed().unwrap());
        let relocated = curve.try_change_closed_seam(7.0).unwrap();
        assert!(!relocated.is_periodic());
        assert_eq!(relocated.domain(), 7.0..=15.0);
        assert_eq!(
            relocated.knots(),
            &[7.0, 7.0, 7.0, 9.0, 12.0, 12.0, 13.0, 15.0, 15.0, 15.0]
        );
        let expected_weights = [
            1.028_571_428_571_428_5,
            1.057_142_857_142_857_2,
            1.7,
            2.0,
            5.2,
            4.0,
            4.114_285_714_285_714,
        ];
        for (control, expected) in relocated.control_points().iter().zip(expected_weights) {
            assert!(Tolerance::DEFAULT.approx_eq(control.weight(), expected));
        }
        for sample in 0..=40 {
            let parameter = 7.0 + sample as Real / 40.0 * 8.0;
            let source_parameter = if parameter <= 12.0 {
                parameter
            } else {
                parameter - 8.0
            };
            assert_point_near(
                relocated.evaluate(parameter).unwrap(),
                curve.evaluate(source_parameter).unwrap(),
            );
        }

        let periodic = NurbsCurve::try_new_rational(
            3,
            [
                ([0.0, 0.0, 0.0], 0.5),
                ([3.0, -1.0, 0.0], 1.2),
                ([5.0, 3.0, 1.0], 0.8),
                ([1.0, 5.0, -1.0], 1.5),
                ([-2.0, 2.0, 0.0], 0.7),
                ([0.0, 0.0, 0.0], 0.5),
                ([3.0, -1.0, 0.0], 1.2),
                ([5.0, 3.0, 1.0], 0.8),
            ]
            .into_iter()
            .map(|(coordinates, weight)| {
                WeightedPoint3::try_new(Point3::try_from(coordinates).unwrap(), weight).unwrap()
            })
            .collect(),
            vec![
                -2.0, -2.0, -1.0, 0.0, 1.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0, 5.0,
            ],
        )
        .unwrap();
        assert!(periodic.is_periodic());
        let clamped = periodic.try_change_closed_seam(1.0).unwrap();
        assert!(!clamped.is_periodic());
        assert_eq!(clamped.domain(), 1.0..=5.0);
        assert_eq!(
            clamped.knots(),
            &[
                1.0, 1.0, 1.0, 1.0, 2.0, 3.0, 4.0, 4.0, 4.0, 5.0, 5.0, 5.0, 5.0
            ]
        );
        assert_eq!(clamped.control_points().len(), 9);
    }

    #[test]
    fn closed_curve_seam_relocation_rejects_open_or_out_of_domain_curves() {
        let open = NurbsCurve::try_new(
            2,
            vec![point(0.0, 0.0), point(2.0, 3.0), point(5.0, 0.0)],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        assert_eq!(
            open.try_change_closed_seam(0.5),
            Err(GeometryError::CurveSeamMustBeClosed)
        );
        let closed = NurbsCurve::try_control_point_curve_with_closure(
            1,
            vec![
                point(0.0, 0.0),
                point(4.0, 0.0),
                point(4.0, 3.0),
                point(0.0, 3.0),
            ],
            ControlPointCurveClosure::Sharp,
        )
        .unwrap();
        assert!(matches!(
            closed.try_change_closed_seam(f64::NAN),
            Err(GeometryError::NonFinite { .. })
        ));
        assert!(matches!(
            closed.try_change_closed_seam(*closed.domain().end() + 1.0),
            Err(GeometryError::ParameterOutOfDomain { .. })
        ));
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
            &[8.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 18.0]
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
    fn make_uniform_preserves_rational_controls_and_resets_clamped_knots() {
        let controls = vec![
            WeightedPoint3::try_new(point(0.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(1.0, 2.0), 0.5).unwrap(),
            WeightedPoint3::try_new(point(3.0, 1.0), 2.0).unwrap(),
            WeightedPoint3::try_new(point(4.0, 0.0), 1.0).unwrap(),
        ];
        let curve = NurbsCurve::try_new_rational(
            2,
            controls.clone(),
            vec![10.0, 10.0, 10.0, 10.2, 11.0, 11.0, 11.0],
        )
        .unwrap();

        let uniform = curve.try_make_uniform().unwrap();

        assert_eq!(uniform.degree(), 2);
        assert_eq!(uniform.control_points(), controls);
        assert_eq!(uniform.knots(), &[0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]);
        assert_eq!(uniform.domain(), 0.0..=2.0);
        assert!(uniform.is_rational());
    }

    #[test]
    fn make_uniform_retains_each_endpoint_clamp_independently() {
        let controls = vec![
            point(0.0, 0.0),
            point(1.0, 3.0),
            point(3.0, -2.0),
            point(6.0, 4.0),
            point(9.0, -1.0),
            point(10.0, 0.0),
        ];
        let cases = [
            (
                vec![0.0, 0.0, 1.0, 2.0, 4.0, 7.0, 10.0, 12.0, 13.0, 13.0],
                vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 7.0],
            ),
            (
                vec![0.0, 0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 10.0, 12.0, 12.0],
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0],
            ),
            (
                vec![0.0, 0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 10.0, 10.0, 10.0],
                vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0, 5.0, 5.0],
            ),
        ];

        for (source_knots, expected_knots) in cases {
            let curve = NurbsCurve::try_new(3, controls.clone(), source_knots).unwrap();
            assert_eq!(curve.try_make_uniform().unwrap().knots(), expected_knots);
        }
    }

    #[test]
    fn make_uniform_retains_periodic_topology() {
        let points = [
            (0.2857142857142857, 2.0),
            (-0.5714285714285714, -0.5714285714285714),
            (2.0, 0.2857142857142857),
            (4.571428571428571, -0.5714285714285714),
            (3.714285714285714, 2.0),
            (4.571428571428571, 4.571428571428571),
            (2.0, 3.714285714285714),
            (-0.5714285714285714, 4.571428571428571),
            (0.2857142857142857, 2.0),
            (-0.5714285714285714, -0.5714285714285714),
            (2.0, 0.2857142857142857),
        ];
        let controls = points
            .into_iter()
            .map(|(x, y)| point(x, y))
            .collect::<Vec<_>>();
        let curve = NurbsCurve::try_new(
            3,
            controls,
            vec![
                -2.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 10.0,
            ],
        )
        .unwrap();
        assert!(curve.is_periodic());

        let uniform = curve.try_make_uniform().unwrap();

        assert_eq!(
            uniform.knots(),
            &[
                0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 12.0
            ]
        );
        assert_eq!(uniform.domain(), 2.0..=10.0);
        assert!(uniform.is_periodic());
        assert!(uniform.is_closed().unwrap());
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
    fn control_point_insertion_matches_rhino_greville_rules() {
        let controls = [
            [0.0, 0.0, 0.0],
            [2.0, 4.0, 1.0],
            [5.0, -1.0, 2.0],
            [7.0, 3.0, -1.0],
            [9.0, 1.0, 0.0],
            [12.0, 5.0, 2.0],
            [15.0, -2.0, 1.0],
        ]
        .into_iter()
        .map(|point| Point3::try_from(point).unwrap())
        .collect::<Vec<_>>();
        let cubic = NurbsCurve::try_new(
            3,
            controls.clone(),
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0],
        )
        .unwrap();

        let inserted = cubic.try_insert_control_point(2.25, false).unwrap();
        assert_eq!(inserted.degree(), 3);
        assert_eq!(
            inserted.knots(),
            &[0.0, 0.0, 0.0, 0.0, 1.0, 2.25, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0,]
        );
        assert_point_near(
            inserted.control_points()[3].point(),
            Point3::try_new(6.1, 1.2, 0.35).unwrap(),
        );
        assert_eq!(inserted.control_points()[3].weight(), 1.0);
        assert_eq!(inserted.control_points()[4].point(), controls[3]);

        let midpoint = cubic.try_insert_control_point(2.25, true).unwrap();
        assert_eq!(
            midpoint.knots(),
            &[
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                13.0 / 6.0,
                3.0,
                5.0,
                8.0,
                8.0,
                8.0,
                8.0,
            ]
        );
        assert_point_near(
            midpoint.control_points()[3].point(),
            Point3::try_new(6.0, 1.0, 0.5).unwrap(),
        );

        assert!(matches!(
            cubic.try_insert_control_point(0.0, false),
            Err(GeometryError::NoControlPointInsertionInterval { .. })
        ));
        let extended = cubic.try_insert_control_point(0.0, true).unwrap();
        assert_eq!(extended.knots()[4], 1.0 / 6.0);
        assert_point_near(
            extended.control_points()[1].point(),
            Point3::try_new(1.0, 2.0, 0.5).unwrap(),
        );

        let quadratic = NurbsCurve::try_new(
            2,
            [
                [0.0, 0.0, 0.0],
                [2.0, 4.0, 1.0],
                [5.0, -1.0, 2.0],
                [8.0, 3.0, -1.0],
                [11.0, 1.0, 0.0],
                [14.0, 5.0, 2.0],
                [17.0, -2.0, 1.0],
                [20.0, 2.0, -2.0],
            ]
            .into_iter()
            .map(|point| Point3::try_from(point).unwrap())
            .collect(),
            vec![0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 7.0, 11.0, 14.0, 14.0, 14.0],
        )
        .unwrap();
        let inserted = quadratic.try_insert_control_point(3.0, false).unwrap();
        assert_eq!(
            inserted.control_points()[3].point(),
            quadratic.control_points()[3].point()
        );
        assert_eq!(
            inserted.control_points()[4].point(),
            quadratic.control_points()[3].point()
        );
        assert_eq!(
            inserted.knots(),
            &[
                0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 7.0, 11.0, 14.0, 14.0, 14.0,
            ]
        );
        let midpoint = quadratic.try_insert_control_point(3.0, true).unwrap();
        assert_eq!(midpoint.knots()[5], 2.25);
        assert_point_near(
            midpoint.control_points()[3].point(),
            Point3::try_new(6.5, 1.0, 0.5).unwrap(),
        );
    }

    #[test]
    fn control_point_insertion_handles_rational_and_periodic_curves() {
        let rational = NurbsCurve::try_new_rational(
            2,
            [
                ([-1.0, 0.0, 0.0], 0.7),
                ([2.0, 5.0, 1.0], 1.6),
                ([6.0, -2.0, 0.0], 0.8),
                ([9.0, 4.0, -1.0], 1.3),
                ([12.0, 0.0, 2.0], 0.9),
            ]
            .into_iter()
            .map(|(point, weight)| {
                WeightedPoint3::try_new(Point3::try_from(point).unwrap(), weight).unwrap()
            })
            .collect(),
            vec![-2.0, -2.0, -2.0, 1.0, 1.0, 6.0, 6.0, 6.0],
        )
        .unwrap()
        .try_insert_control_point(2.0, false)
        .unwrap();
        assert_eq!(
            rational.knots(),
            &[-2.0, -2.0, -2.0, 1.0, 1.0, 2.0, 6.0, 6.0, 6.0]
        );
        assert_point_near(
            rational.control_points()[3].point(),
            Point3::try_new(7.2, 0.4, -0.4).unwrap(),
        );
        assert_eq!(rational.control_points()[3].weight(), 1.0);
        assert_eq!(
            rational
                .control_points()
                .iter()
                .map(|control| control.weight())
                .collect::<Vec<_>>(),
            vec![0.7, 1.6, 0.8, 1.0, 1.3, 0.9]
        );

        let unique = [
            point(0.0, 0.0),
            point(2.0, 0.0),
            point(2.0, 2.0),
            point(0.0, 2.0),
        ];
        let periodic = NurbsCurve::try_new(
            3,
            unique.iter().chain(&unique[..3]).copied().collect(),
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
        )
        .unwrap()
        .try_insert_control_point(3.4, false)
        .unwrap();
        assert!(periodic.is_periodic());
        assert_eq!(
            periodic.knots(),
            &[
                -2.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 7.0,
            ]
        );
        let expected_unique = [unique[0], unique[1], unique[2], point(1.2, 2.0), unique[3]];
        for (actual, expected) in periodic
            .control_points()
            .iter()
            .zip(expected_unique.iter().chain(&expected_unique[..3]))
        {
            assert_point_near(actual.point(), *expected);
        }
    }

    #[test]
    fn control_point_removal_matches_rhino_knot_and_degree_rules() {
        let cubic_controls = [
            point(0.0, 0.0),
            point(2.0, 4.0),
            point(5.0, -1.0),
            point(7.0, 3.0),
            point(9.0, 1.0),
            point(12.0, 5.0),
            point(15.0, -2.0),
        ];
        let cubic = NurbsCurve::try_new(
            3,
            cubic_controls.to_vec(),
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0],
        )
        .unwrap();
        for (index, expected_knots) in [
            vec![0.0, 0.0, 0.0, 0.0, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0],
            vec![0.0, 0.0, 0.0, 0.0, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0],
            vec![0.0, 0.0, 0.0, 0.0, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 5.0, 8.0, 8.0, 8.0, 8.0],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 8.0, 8.0, 8.0, 8.0],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 8.0, 8.0, 8.0, 8.0],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 8.0, 8.0, 8.0, 8.0],
        ]
        .into_iter()
        .enumerate()
        {
            let removed = cubic.try_remove_control_point(index).unwrap();
            assert_eq!(removed.knots(), expected_knots);
            let expected_controls = cubic_controls
                .iter()
                .enumerate()
                .filter_map(|(control, point)| (control != index).then_some(*point))
                .collect::<Vec<_>>();
            assert_eq!(
                removed
                    .control_points()
                    .iter()
                    .map(|control| control.point())
                    .collect::<Vec<_>>(),
                expected_controls
            );
        }

        let quadratic = NurbsCurve::try_new(
            2,
            (0..8).map(|index| point(index as Real, 0.0)).collect(),
            vec![0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 7.0, 11.0, 14.0, 14.0, 14.0],
        )
        .unwrap();
        assert_eq!(
            quadratic.try_remove_control_point(3).unwrap().knots(),
            &[0.0, 0.0, 0.0, 1.5, 4.0, 7.0, 11.0, 14.0, 14.0, 14.0]
        );
        assert_eq!(
            quadratic.try_remove_control_point(5).unwrap().knots(),
            &[0.0, 0.0, 0.0, 1.0, 2.0, 5.5, 11.0, 14.0, 14.0, 14.0]
        );

        let bezier = NurbsCurve::try_new(
            3,
            cubic_controls[..4].to_vec(),
            vec![0.0, 0.0, 0.0, 0.0, 7.0, 7.0, 7.0, 7.0],
        )
        .unwrap()
        .try_remove_control_point(1)
        .unwrap();
        assert_eq!(bezier.degree(), 2);
        assert_eq!(bezier.knots(), &[0.0, 0.0, 0.0, 7.0, 7.0, 7.0]);
        assert_eq!(
            bezier
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            vec![cubic_controls[0], cubic_controls[2], cubic_controls[3]]
        );
    }

    #[test]
    fn control_point_removal_handles_rational_and_periodic_curves() {
        let rational = NurbsCurve::try_new_rational(
            2,
            [
                ([-1.0, 0.0, 0.0], 0.7),
                ([2.0, 5.0, 1.0], 1.6),
                ([6.0, -2.0, 0.0], 0.8),
                ([9.0, 4.0, -1.0], 1.3),
                ([12.0, 0.0, 2.0], 0.9),
            ]
            .into_iter()
            .map(|(point, weight)| {
                WeightedPoint3::try_new(Point3::try_from(point).unwrap(), weight).unwrap()
            })
            .collect(),
            vec![-2.0, -2.0, -2.0, 1.0, 1.0, 6.0, 6.0, 6.0],
        )
        .unwrap()
        .try_remove_control_point(2)
        .unwrap();
        assert_eq!(rational.knots(), &[-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0]);
        assert_eq!(
            rational
                .control_points()
                .iter()
                .map(|control| control.weight())
                .collect::<Vec<_>>(),
            vec![1.0, 1.6, 1.3, 1.0]
        );

        let unique = [
            point(0.0, 0.0),
            point(2.0, 0.0),
            point(2.0, 2.0),
            point(0.0, 2.0),
        ];
        let periodic = NurbsCurve::try_new(
            3,
            unique.iter().chain(&unique[..3]).copied().collect(),
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
        )
        .unwrap()
        .try_remove_control_point(1)
        .unwrap();
        assert!(periodic.is_periodic());
        assert_eq!(
            periodic.knots(),
            &[-2.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0]
        );
        let remaining = [unique[0], unique[2], unique[3]];
        assert_eq!(
            periodic
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            remaining
                .iter()
                .chain(&remaining)
                .copied()
                .collect::<Vec<_>>()
        );

        let minimal_quadratic = NurbsCurve::try_new(
            2,
            unique[..3].iter().chain(&unique[..2]).copied().collect(),
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0],
        )
        .unwrap()
        .try_remove_control_point(1)
        .unwrap();
        assert_eq!(minimal_quadratic.degree(), 2);
        assert!(minimal_quadratic.is_periodic());
        assert_eq!(
            minimal_quadratic.knots(),
            &[-1.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0]
        );
        assert_eq!(
            minimal_quadratic
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            unique[..3]
                .iter()
                .chain(&unique[..2])
                .copied()
                .collect::<Vec<_>>()
        );

        let minimal_quartic = NurbsCurve::try_new(
            4,
            unique.iter().chain(&unique).copied().collect(),
            vec![
                -3.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 7.0,
            ],
        )
        .unwrap()
        .try_remove_control_point(1)
        .unwrap();
        assert_eq!(minimal_quartic.degree(), 3);
        assert!(minimal_quartic.is_periodic());
        assert_eq!(
            minimal_quartic.knots(),
            &[-2.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0]
        );
        let remaining = [unique[0], unique[2], unique[3]];
        assert_eq!(
            minimal_quartic
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            remaining
                .iter()
                .chain(&remaining)
                .copied()
                .collect::<Vec<_>>()
        );

        assert!(matches!(
            periodic.try_remove_control_point(3),
            Err(GeometryError::ControlPointIndexOutOfRange { .. })
        ));
        let line = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(1.0, 0.0)],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        assert!(matches!(
            line.try_remove_control_point(0),
            Err(GeometryError::InsufficientControlPoints { .. })
        ));
    }

    #[test]
    fn knot_removal_matches_rhino_nearest_and_greville_rules() {
        let source = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 0.0),
                point(2.0, 4.0),
                point(5.0, -1.0),
                point(7.0, 3.0),
                point(9.0, 1.0),
                point(12.0, 5.0),
                point(15.0, -2.0),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0],
        )
        .unwrap();

        // Parameter 1.95 is numerically nearer knot one, but its curve point
        // is nearer the point at knot three; Rhino therefore removes three.
        let removed = source.try_remove_knot_near(1.95).unwrap();
        assert_eq!(removed.degree(), 3);
        assert_eq!(
            removed.knots(),
            &[0.0, 0.0, 0.0, 0.0, 1.0, 5.0, 8.0, 8.0, 8.0, 8.0]
        );
        let expected = [
            [0.0, 0.0, 0.0],
            [2.0145063256868987, 3.71309711419245, 0.0],
            [6.824390238905354, -0.8601625027947495, 0.0],
            [7.66282653203088, 2.112986366500404, 0.0],
            [12.02374775546043, 4.5303221697825595, 0.0],
            [15.0, -2.0, 0.0],
        ];
        for (control, expected) in removed.control_points().iter().zip(expected) {
            assert_point_near(control.point(), Point3::try_from(expected).unwrap());
        }

        assert!(matches!(
            source.try_remove_knot_near(0.1),
            Err(GeometryError::NoRemovableKnot { parameter: 0.1 })
        ));
        assert!(matches!(
            source.try_remove_knot_near(Real::NAN),
            Err(GeometryError::NonFinite { .. })
        ));

        let periodic = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(3.0, -1.0),
                point(5.0, 4.0),
                point(0.0, 0.0),
                point(3.0, -1.0),
            ],
            vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        )
        .unwrap();
        assert!(periodic.is_periodic());
        assert_eq!(
            periodic.try_remove_knot_near(1.0),
            Err(GeometryError::PeriodicKnotRemovalUnsupported { direction: "curve" })
        );
    }

    #[test]
    fn knot_removal_handles_repeated_rational_knots() {
        let source = NurbsCurve::try_new_rational(
            2,
            [
                ([-1.0, 0.0, 0.0], 0.7),
                ([2.0, 5.0, 1.0], 1.6),
                ([6.0, -2.0, 0.0], 0.8),
                ([9.0, 4.0, -1.0], 1.3),
                ([12.0, 0.0, 2.0], 0.9),
            ]
            .into_iter()
            .map(|(point, weight)| {
                WeightedPoint3::try_new(Point3::try_from(point).unwrap(), weight).unwrap()
            })
            .collect(),
            vec![-2.0, -2.0, -2.0, 1.0, 1.0, 6.0, 6.0, 6.0],
        )
        .unwrap();
        let removed = source.try_remove_knot_near(1.0).unwrap();
        assert_eq!(removed.knots(), &[-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0]);
        let expected_weights = [0.7, 1.3708333333333331, 1.0708333333333335, 0.9];
        for (control, expected) in removed.control_points().iter().zip(expected_weights) {
            assert!((control.weight() - expected).abs() <= 2.0e-15);
        }

        let non_clamped = NurbsCurve::try_new_rational(
            2,
            [
                ([0.0, 0.0, 0.0], 0.5),
                ([2.0, 4.0, 1.0], 1.1),
                ([5.0, -1.0, 2.0], 0.8),
                ([8.0, 2.0, 0.0], 1.4),
            ]
            .into_iter()
            .map(|(point, weight)| {
                WeightedPoint3::try_new(Point3::try_from(point).unwrap(), weight).unwrap()
            })
            .collect(),
            vec![8.0, 9.0, 10.0, 11.5, 14.0, 15.0, 16.0],
        )
        .unwrap();
        let endpoints = [
            non_clamped.evaluate(10.0).unwrap(),
            non_clamped.evaluate(14.0).unwrap(),
        ];
        let removed = non_clamped.try_remove_knot_near(11.3).unwrap();
        assert_eq!(removed.domain(), 10.0..=14.0);
        assert_eq!(removed.knots(), &[10.0, 10.0, 10.0, 14.0, 14.0, 14.0]);
        assert_point_near(removed.evaluate(10.0).unwrap(), endpoints[0]);
        assert_point_near(removed.evaluate(14.0).unwrap(), endpoints[1]);
    }

    #[test]
    fn multiple_knot_removal_matches_rhino_group_order_and_kink_filter() {
        let source = NurbsCurve::try_new(
            3,
            [
                [0.0, 0.0, 0.0],
                [1.0, 3.0, 1.0],
                [3.0, -2.0, 2.0],
                [5.0, 4.0, -1.0],
                [7.0, 0.0, 1.0],
                [9.0, -3.0, 2.0],
                [11.0, 5.0, -2.0],
                [13.0, 1.0, 0.0],
                [15.0, -1.0, 3.0],
                [18.0, 2.0, 1.0],
            ]
            .into_iter()
            .map(|point| Point3::try_from(point).unwrap())
            .collect(),
            vec![
                0.0, 0.0, 0.0, 0.0, 2.0, 2.0, 5.0, 5.0, 5.0, 7.0, 10.0, 10.0, 10.0, 10.0,
            ],
        )
        .unwrap();

        let (ordinary, removed) = source.try_remove_multiple_knots(false, 0.0).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(
            ordinary.knots(),
            &[
                0.0, 0.0, 0.0, 0.0, 2.0, 5.0, 5.0, 5.0, 7.0, 10.0, 10.0, 10.0, 10.0
            ]
        );
        assert_point_near(
            ordinary.control_points()[1].point(),
            Point3::try_new(1.2511378848728234, 1.2599732262382861, 1.627844712182061).unwrap(),
        );

        let (none, removed) = source.try_remove_multiple_knots(true, 0.0).unwrap();
        assert_eq!((none, removed), (source.clone(), 0));

        let (below_kink, removed) = source
            .try_remove_multiple_knots(true, 130.0_f64.to_radians())
            .unwrap();
        assert_eq!(removed, 1);
        assert_eq!(below_kink.knots(), ordinary.knots());

        let (all, removed) = source
            .try_remove_multiple_knots(true, 135.0_f64.to_radians())
            .unwrap();
        assert_eq!(removed, 3);
        assert_eq!(
            all.knots(),
            &[0.0, 0.0, 0.0, 0.0, 2.0, 5.0, 7.0, 10.0, 10.0, 10.0, 10.0]
        );
        assert_point_near(
            all.control_points()[1].point(),
            Point3::try_new(1.277439372269475, 0.7925502880770412, 1.80206788629584).unwrap(),
        );

        assert_eq!(
            source.try_remove_multiple_knots(true, -1.0),
            Err(GeometryError::InvalidKnotRemovalAngle)
        );
        assert_eq!(
            source.try_remove_multiple_knots(true, Real::NAN),
            Err(GeometryError::InvalidKnotRemovalAngle)
        );

        let non_clamped = NurbsCurve::try_new(
            3,
            [
                [0.0, 0.0, 0.0],
                [1.0, 3.0, 1.0],
                [3.0, -2.0, 2.0],
                [6.0, 4.0, -1.0],
                [8.0, 0.0, 1.0],
                [11.0, 2.0, 0.0],
            ]
            .into_iter()
            .map(|point| Point3::try_from(point).unwrap())
            .collect(),
            vec![0.0, 1.0, 2.0, 3.0, 4.0, 4.0, 6.0, 7.0, 8.0, 9.0],
        )
        .unwrap();
        let endpoints = [
            non_clamped.evaluate(3.0).unwrap(),
            non_clamped.evaluate(6.0).unwrap(),
        ];
        let (non_clamped, removed) = non_clamped.try_remove_multiple_knots(false, 0.0).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(non_clamped.domain(), 3.0..=6.0);
        assert_eq!(
            non_clamped.knots(),
            &[3.0, 3.0, 3.0, 3.0, 4.0, 6.0, 6.0, 6.0, 6.0]
        );
        assert_point_near(non_clamped.evaluate(3.0).unwrap(), endpoints[0]);
        assert_point_near(non_clamped.evaluate(6.0).unwrap(), endpoints[1]);

        let linear = NurbsCurve::try_new(
            1,
            [
                [0.0, 0.0, 0.0],
                [2.0, 3.0, 0.0],
                [5.0, -1.0, 1.0],
                [9.0, 2.0, 0.0],
                [12.0, 0.0, 2.0],
            ]
            .into_iter()
            .map(|point| Point3::try_from(point).unwrap())
            .collect(),
            vec![0.0, 0.0, 2.0, 4.0, 6.0, 8.0, 8.0],
        )
        .unwrap();
        let (linear, removed) = linear
            .try_remove_multiple_knots(true, std::f64::consts::PI)
            .unwrap();
        assert_eq!(removed, 3);
        assert_eq!(linear.knots(), &[0.0, 0.0, 8.0, 8.0]);
        assert_eq!(linear.control_points().len(), 2);
    }

    #[test]
    fn knot_insertion_restores_periodic_controls_and_knot_intervals() {
        let source = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 0.0),
                point(2.0, 0.0),
                point(2.0, 2.0),
                point(0.0, 2.0),
                point(0.0, 0.0),
                point(2.0, 0.0),
                point(2.0, 2.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
        )
        .unwrap();
        assert!(source.is_periodic());

        let cases = [
            (
                4.5,
                vec![0.5, 0.5, 1.0, 2.0, 3.0, 4.0, 4.5, 5.0, 6.0, 7.0, 8.0, 8.0],
            ),
            (
                2.25,
                vec![0.0, 0.0, 1.0, 2.0, 2.25, 3.0, 4.0, 5.0, 6.0, 6.25, 7.0, 7.0],
            ),
        ];
        for (parameter, expected_knots) in cases {
            let refined = source.try_insert_knot(parameter, 1).unwrap();
            assert_eq!(refined.knots(), expected_knots);
            assert!(refined.is_periodic());
            assert!(refined.is_closed().unwrap());
            let repeat_start = refined.control_points().len() - refined.degree();
            assert_eq!(
                &refined.control_points()[..refined.degree()],
                &refined.control_points()[repeat_start..]
            );
            for sample in 0..=64 {
                let normalized = sample as Real / 64.0;
                let source_parameter = source.parameter_at(normalized).unwrap();
                let refined_parameter = refined.parameter_at(normalized).unwrap();
                assert_point_near(
                    refined.evaluate(refined_parameter).unwrap(),
                    source.evaluate(source_parameter).unwrap(),
                );
            }
        }
    }

    #[test]
    fn make_non_periodic_clamps_a_periodic_curve_without_changing_its_locus() {
        let source = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 0.0),
                point(2.0, 0.0),
                point(2.0, 2.0),
                point(0.0, 2.0),
                point(0.0, 0.0),
                point(2.0, 0.0),
                point(2.0, 2.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
        )
        .unwrap();
        let domain = source.domain();
        let clamped = source.try_make_non_periodic().unwrap();
        assert!(!clamped.is_periodic());
        assert!(clamped.is_closed().unwrap());
        assert_eq!(clamped.domain(), domain);
        assert!(
            clamped.knots()[..=clamped.degree()]
                .iter()
                .all(|knot| *knot == *domain.start())
        );
        assert!(
            clamped.knots()[clamped.knots().len() - clamped.degree() - 1..]
                .iter()
                .all(|knot| *knot == *domain.end())
        );
        for sample in 0..=64 {
            let parameter = source.parameter_at(sample as Real / 64.0).unwrap();
            assert_point_near(
                clamped.evaluate(parameter).unwrap(),
                source.evaluate(parameter).unwrap(),
            );
        }
        assert_eq!(clamped.try_make_non_periodic().unwrap(), clamped);
    }

    #[test]
    fn change_degree_matches_rhino_knot_and_greville_interpolation_rules() {
        let source = NurbsCurve::try_new(
            3,
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 4.0, 1.0).unwrap(),
                Point3::try_new(5.0, -1.0, 2.0).unwrap(),
                Point3::try_new(7.0, 3.0, -1.0).unwrap(),
                Point3::try_new(9.0, 1.0, 0.0).unwrap(),
                Point3::try_new(12.0, 5.0, 2.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 7.0, 7.0, 7.0, 7.0],
        )
        .unwrap();

        let elevated = source.try_change_degree(5, false).unwrap();
        assert_eq!(elevated.degree(), 5);
        assert_eq!(elevated.control_points().len(), 12);
        assert_eq!(
            elevated.knots(),
            &[
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 3.0, 3.0, 3.0, 7.0, 7.0, 7.0, 7.0,
                7.0, 7.0,
            ]
        );
        for sample in 0..=64 {
            let parameter = source.parameter_at(sample as Real / 64.0).unwrap();
            assert_point_near(
                elevated.evaluate(parameter).unwrap(),
                source.evaluate(parameter).unwrap(),
            );
        }

        let reduced = source.try_change_degree(2, false).unwrap();
        assert_eq!(reduced.knots(), &[0.0, 0.0, 0.0, 1.0, 3.0, 7.0, 7.0, 7.0]);
        let expected = [
            [0.0, 0.0, 0.0],
            [2.795509342977698, 3.882157926461724, 1.4232971669680532],
            [5.778782399035563, -0.4382157926461724, 1.2326702833031946],
            [7.876130198915011, 1.465340566606389, -1.1549125979505726],
            [12.0, 5.0, 2.0],
        ];
        for (control, expected) in reduced.control_points().iter().zip(expected) {
            assert_point_near(control.point(), Point3::try_from(expected).unwrap());
        }

        let deformable = source.try_change_degree(5, true).unwrap();
        assert_eq!(
            deformable.knots(),
            &[
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0
            ]
        );
        assert_point_near(
            deformable.control_points()[3].point(),
            Point3::try_new(6.353798126845488, -3.487896224537079, 1.5666404989296816).unwrap(),
        );
        assert_eq!(source.try_change_degree(3, true).unwrap(), source);
        assert_eq!(
            source.try_change_degree(0, false),
            Err(GeometryError::InvalidDegree)
        );
    }

    #[test]
    fn signed_rational_weights_evaluate_and_refine_projectively() {
        assert!(WeightedPoint3::try_new(point(0.0, 0.0), -0.2).is_ok());
        assert!(WeightedPoint3::try_new(point(0.0, 0.0), 0.0).is_err());
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(0.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(2.0, 3.0), -0.2).unwrap(),
                WeightedPoint3::try_new(point(5.0, 0.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        assert_point_near(curve.evaluate(0.5).unwrap(), point(2.625, -0.75));

        let refined = curve.try_insert_knot(0.25, 1).unwrap();
        for sample in 0..=32 {
            let parameter = sample as Real / 32.0;
            assert_point_near(
                refined.evaluate(parameter).unwrap(),
                curve.evaluate(parameter).unwrap(),
            );
        }
    }

    #[test]
    fn make_periodic_without_smoothing_matches_rhino_control_rotation_and_knots() {
        let source = NurbsCurve::try_new(
            3,
            vec![
                point(1.0, 0.0),
                point(4.0, -1.0),
                Point3::try_new(6.0, 2.0, 1.0).unwrap(),
                point(4.0, 5.0),
                Point3::try_new(0.0, 4.0, -1.0).unwrap(),
                point(-2.0, 1.0),
                point(1.0, 0.0),
            ],
            vec![
                10.0, 10.0, 10.0, 10.0, 11.0, 13.0, 19.0, 25.0, 25.0, 25.0, 25.0,
            ],
        )
        .unwrap();

        let periodic = source.try_make_periodic(false).unwrap();

        assert!(periodic.is_periodic());
        assert!(periodic.is_closed().unwrap());
        assert_eq!(periodic.domain(), 10.0..=25.0);
        let expected_points = [
            [-2.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [4.0, -1.0, 0.0],
            [6.0, 2.0, 1.0],
            [4.0, 5.0, 0.0],
            [0.0, 4.0, -1.0],
            [-2.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [4.0, -1.0, 0.0],
        ];
        assert_eq!(periodic.control_points().len(), expected_points.len());
        for (control, expected) in periodic.control_points().iter().zip(expected_points) {
            assert_point_near(control.point(), Point3::try_from(expected).unwrap());
            assert_eq!(control.weight(), 1.0);
        }
        let expected_knots = [
            5.227272727272728,
            5.227272727272728,
            7.613636363636364,
            10.0,
            12.386363636363637,
            14.772727272727273,
            16.136363636363637,
            20.227272727272727,
            22.613636363636363,
            25.0,
            27.386363636363637,
            29.772727272727273,
            29.772727272727273,
        ];
        for (actual, expected) in periodic.knots().iter().zip(expected_knots) {
            assert!((actual - expected).abs() <= 4.0e-15);
        }
    }

    #[test]
    fn make_periodic_without_smoothing_handles_single_span_odd_degrees() {
        let cubic = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 0.0),
                point(5.0, -2.0),
                Point3::try_new(4.0, 6.0, 1.0).unwrap(),
                point(0.0, 0.0),
            ],
            vec![3.0, 3.0, 3.0, 3.0, 11.0, 11.0, 11.0, 11.0],
        )
        .unwrap();

        let periodic = cubic.try_make_periodic(false).unwrap();

        assert!(periodic.is_periodic());
        assert_eq!(periodic.control_points().len(), 6);
        let expected_knots = [
            -2.333333333333333,
            -2.333333333333333,
            0.3333333333333335,
            3.0,
            5.666666666666666,
            8.333333333333334,
            11.0,
            13.666666666666666,
            16.333333333333332,
            16.333333333333332,
        ];
        for (actual, expected) in periodic.knots().iter().zip(expected_knots) {
            assert!((actual - expected).abs() <= 4.0e-15);
        }
    }

    #[test]
    fn make_periodic_with_smoothing_matches_rhino_homogeneous_interpolation() {
        let source = NurbsCurve::try_new_rational(
            2,
            [
                ([-1.0, 0.0, 0.0], 0.75),
                ([2.0, -2.0, 0.0], 1.5),
                ([4.0, 2.0, 1.0], 0.6),
                ([0.0, 4.0, 0.0], 1.8),
                ([-1.0, 0.0, 0.0], 0.75),
            ]
            .into_iter()
            .map(|(point, weight)| {
                WeightedPoint3::try_new(Point3::try_from(point).unwrap(), weight).unwrap()
            })
            .collect(),
            vec![0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 8.0, 8.0],
        )
        .unwrap();

        let periodic = source.try_make_periodic(true).unwrap();

        assert!(periodic.is_periodic());
        assert!(periodic.is_closed().unwrap());
        assert_eq!(periodic.domain(), source.domain());
        assert_eq!(
            periodic.knots(),
            &[-3.0, -3.0, 0.0, 2.0, 5.0, 8.0, 10.0, 10.0]
        );
        let expected = [
            (
                [-0.5057939242092077, 4.476041340432196, 0.0],
                1.5350961538461538,
            ),
            (
                [1.8129330254041567, -2.651270207852194, 0.0],
                1.2490384615384615,
            ),
            (
                [3.8504479669193663, 1.893866299104066, 0.8600964851826325],
                0.697596153846154,
            ),
            (
                [-0.5057939242092077, 4.476041340432196, 0.0],
                1.5350961538461538,
            ),
            (
                [1.8129330254041567, -2.651270207852194, 0.0],
                1.2490384615384615,
            ),
        ];
        for (control, (expected_point, expected_weight)) in
            periodic.control_points().iter().zip(expected)
        {
            assert_point_near(control.point(), Point3::try_from(expected_point).unwrap());
            assert!((control.weight() - expected_weight).abs() <= 2.0e-15);
        }
    }

    #[test]
    fn smooth_periodic_greville_phase_starts_with_the_first_in_domain_value() {
        let source_knots = [0.0, 0.0, 0.0, 0.0, 0.1, 9.0, 9.5, 10.0, 10.0, 10.0, 10.0];
        let knots = periodic_knots_preserving_active(3, 7, &source_knots).unwrap();
        let parameters = periodic_greville_parameters(3, 7, 4, &knots).unwrap();
        let expected = [3.033333333333333, 6.2, 9.5, 9.866666666666667];
        for (actual, expected) in parameters.into_iter().zip(expected) {
            assert!((actual - expected).abs() <= 2.0e-15);
        }

        let source_knots = [0.0, 0.0, 0.0, 9.8, 9.9, 10.0, 10.0, 10.0];
        let knots = periodic_knots_preserving_active(2, 5, &source_knots).unwrap();
        let parameters = periodic_greville_parameters(2, 5, 3, &knots).unwrap();
        assert!((parameters[0] - 4.9).abs() <= 4.0e-15);
        assert!((parameters[1] - 9.85).abs() <= 4.0e-15);
        assert!((parameters[2] - 9.95).abs() <= 4.0e-15);
    }

    #[test]
    fn make_periodic_validates_degree_closure_and_smooth_control_count() {
        let linear_loop = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(2.0, 0.0), point(0.0, 0.0)],
            vec![0.0, 0.0, 1.0, 2.0, 2.0],
        )
        .unwrap();
        assert_eq!(
            linear_loop.try_make_periodic(false),
            Err(GeometryError::PeriodicNurbsDegreeTooLow)
        );

        let open = NurbsCurve::try_clamped_uniform(
            2,
            vec![point(0.0, 0.0), point(1.0, 2.0), point(3.0, 0.0)],
        )
        .unwrap();
        assert_eq!(
            open.try_make_periodic(false),
            Err(GeometryError::PeriodicCurveMustBeClosed)
        );

        let short_quadratic = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0),
                point(5.0, -2.0),
                Point3::try_new(4.0, 6.0, 1.0).unwrap(),
                point(0.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 9.9, 10.0, 10.0, 10.0],
        )
        .unwrap();
        assert_eq!(
            short_quadratic.try_make_periodic(true),
            Err(GeometryError::InsufficientSmoothPeriodicControlPoints {
                degree: 2,
                required: 5,
                actual: 4,
            })
        );

        let short_cubic = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 0.0),
                point(4.0, 0.0),
                point(5.0, 3.0),
                point(0.0, 4.0),
                point(0.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0],
        )
        .unwrap();
        assert_eq!(
            short_cubic.try_make_periodic(true),
            Err(GeometryError::InsufficientSmoothPeriodicControlPoints {
                degree: 3,
                required: 6,
                actual: 5,
            })
        );
        assert!(short_cubic.try_make_periodic(false).unwrap().is_periodic());
    }

    #[test]
    fn make_periodic_is_a_no_op_for_periodic_curves() {
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
        assert_eq!(periodic.try_make_periodic(false).unwrap(), periodic);
        assert_eq!(periodic.try_make_periodic(true).unwrap(), periodic);
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
            assert_eq!(refined.knots()[0], refined.knots()[1]);
            assert_eq!(
                refined.knots()[refined.knots().len() - 1],
                refined.knots()[refined.knots().len() - 2]
            );
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
