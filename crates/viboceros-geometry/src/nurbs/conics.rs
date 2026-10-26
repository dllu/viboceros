//! Conic construction from endpoint, tangent, vertex and focus constraints.
use super::*;

impl NurbsCurve {
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
}
