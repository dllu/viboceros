use std::ops::RangeInclusive;
mod evaluate;
mod weights;
pub(crate) use weights::rescale_controls;

use faer::{Mat, prelude::*};

use crate::{
    AffineTransform3, BoundingBox3, Brep, CircularArc3, Frame3, GeometryError, NurbsSurface,
    Point3, Polyline3, Real, Tolerance, Vector3, integration::integrate_adaptive,
    intersection::curve_surface_intersections, require_finite,
};
use crate::{CurveRef, curve::ArcLengthSampler};

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

/// Endpoint selection for a curve extension by model-space arc length.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveExtensionSide {
    Start,
    End,
    Both,
}

/// Continuation geometry used when extending a curve.
///
/// `Natural` follows Rhino's command behavior: line-like curves continue as
/// lines, exact circular curves retain their circle, and all other curves use
/// smooth NURBS extrapolation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveExtensionStyle {
    Natural,
    Arc,
    Line,
    Smooth,
}

/// Finite geometry that can stop a curve extension.
#[derive(Clone, Debug, PartialEq)]
pub enum CurveExtensionBoundary {
    Curve(NurbsCurve),
    Surface(NurbsSurface),
    Brep(Brep),
}

impl CurveExtensionBoundary {
    fn control_bounds(&self) -> BoundingBox3 {
        match self {
            Self::Curve(curve) => curve.control_point_bounds(),
            Self::Surface(surface) => surface.control_point_bounds(),
            Self::Brep(brep) => brep.bounds(),
        }
    }
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

/// A point where two finite NURBS curves meet within the requested tolerance.
///
/// Collinear overlaps are represented by one record at each overlap endpoint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveCurveIntersection {
    pub(crate) first_parameter: Real,
    pub(crate) second_parameter: Real,
    pub(crate) point: Point3,
}

impl CurveCurveIntersection {
    /// Parameter on the curve on which [`NurbsCurve::intersections_with_curve`]
    /// was invoked.
    #[inline]
    pub const fn first_parameter(self) -> Real {
        self.first_parameter
    }

    /// Parameter on the curve passed to
    /// [`NurbsCurve::intersections_with_curve`].
    #[inline]
    pub const fn second_parameter(self) -> Real {
        self.second_parameter
    }

    /// Midpoint of the two refined curve evaluations.
    #[inline]
    pub const fn point(self) -> Point3 {
        self.point
    }
}

/// A finite interval shared by two NURBS curves.
///
/// `start` and `end` are ordered by the first curve's parameter. The matching
/// parameters on the second curve can decrease when the curves traverse the
/// overlap in opposite directions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveCurveOverlap {
    start: CurveCurveIntersection,
    end: CurveCurveIntersection,
}

impl CurveCurveOverlap {
    /// First overlap boundary, ordered by the first curve's parameter.
    #[inline]
    pub const fn start(self) -> CurveCurveIntersection {
        self.start
    }

    /// Second overlap boundary, ordered by the first curve's parameter.
    #[inline]
    pub const fn end(self) -> CurveCurveIntersection {
        self.end
    }

    /// Increasing parameter interval occupied on the first curve.
    #[inline]
    pub fn first_interval(self) -> RangeInclusive<Real> {
        self.start.first_parameter..=self.end.first_parameter
    }

    /// Increasing parameter interval occupied on the second curve.
    #[inline]
    pub fn second_interval(self) -> RangeInclusive<Real> {
        self.start.second_parameter.min(self.end.second_parameter)
            ..=self.start.second_parameter.max(self.end.second_parameter)
    }
}

/// A finite point contact or shared interval between two NURBS curves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurveCurveIntersectionEvent {
    Point(CurveCurveIntersection),
    Overlap(CurveCurveOverlap),
}

#[derive(Clone, Debug)]
struct CurveIntersectionNode {
    curve: NurbsCurve,
    bounds: BoundingBox3,
    convex_hull_bounds: bool,
    depth: u8,
}

impl CurveIntersectionNode {
    fn try_new(curve: NurbsCurve, depth: u8) -> Result<Self, GeometryError> {
        let first_positive = curve.control_points[0].weight.is_sign_positive();
        let convex_hull_bounds = curve
            .control_points
            .iter()
            .all(|control| control.weight.is_sign_positive() == first_positive);
        let bounds = curve.control_point_bounds();
        Ok(Self {
            curve,
            bounds,
            convex_hull_bounds,
            depth,
        })
    }

    fn split(self) -> Result<[Self; 2], GeometryError> {
        let domain = self.curve.domain();
        let middle = finite_midpoint(*domain.start(), *domain.end());
        let (left, right) = self.curve.try_split(middle)?;
        Ok([
            Self::try_new(left, self.depth + 1)?,
            Self::try_new(right, self.depth + 1)?,
        ])
    }

    fn spatial_size(&self) -> Result<Real, GeometryError> {
        self.bounds.min().distance_to(self.bounds.max())
    }
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

    /// Reports whether an interior knot has full degree multiplicity and a
    /// one-sided tangent discontinuity larger than the angular tolerance.
    ///
    /// These are the curve locations that become distinct B-rep edges when
    /// Rhino uses the curve as a surface cutting object. Lower-multiplicity
    /// knots and collinear full-multiplicity knots remain within one edge.
    pub fn has_full_multiplicity_kink(&self, tolerance: Tolerance) -> Result<bool, GeometryError> {
        Ok(!self
            .full_multiplicity_kink_parameters(tolerance)?
            .is_empty())
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
        if source == domain {
            return Ok(self.clone());
        }
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

    /// Projectively reparameterizes a clamped curve so its two end weights are
    /// exactly one.
    ///
    /// The control locations, parameter domain, and geometric locus remain
    /// unchanged. Interior knots and the correspondence between parameters
    /// and points generally change according to the same Möbius transform
    /// used by OpenNURBS' `ChangeEndWeights` operation.
    pub fn try_normalized_end_weights(&self) -> Result<Self, GeometryError> {
        let curve = self.clamped_to_active_domain()?;
        let control_count = curve.control_points.len();
        let start_weight = curve.control_points[0].weight();
        let end_weight = curve.control_points[control_count - 1].weight();
        if start_weight.is_sign_positive() != end_weight.is_sign_positive() {
            return Err(GeometryError::InvalidControlNet {
                context: "NURBS endpoint weights must have the same sign",
            });
        }

        let start_scale = 1.0 / start_weight;
        let end_scale = 1.0 / end_weight;
        require_finite(
            [start_scale, end_scale],
            "NURBS endpoint-weight normalization scales",
        )?;
        if (start_scale - end_scale).abs() <= end_scale.abs() * Real::EPSILON.sqrt() {
            let scale = if start_scale == end_scale {
                end_scale
            } else {
                start_scale.mul_add(0.5, end_scale * 0.5)
            };
            let mut controls = curve
                .control_points
                .iter()
                .map(|control| WeightedPoint3::try_new(control.point(), control.weight() * scale))
                .collect::<Result<Vec<_>, _>>()?;
            controls[0] = WeightedPoint3::try_new(controls[0].point(), 1.0)?;
            controls[control_count - 1] =
                WeightedPoint3::try_new(controls[control_count - 1].point(), 1.0)?;
            return Self::try_new_rational(curve.degree, controls, curve.knots);
        }

        let degree = curve.degree as Real;
        let log_c = (end_weight.abs().ln() - start_weight.abs().ln()) / degree;
        let c = log_c.exp();
        if !c.is_finite() || c == 0.0 {
            return Err(GeometryError::NonFinite {
                context: "NURBS endpoint-weight Möbius factor",
            });
        }
        let domain = curve.domain();
        let domain_start = *domain.start();
        let domain_end = *domain.end();
        let normalized_knots = curve
            .knots
            .iter()
            .map(|knot| {
                let normalized = reparameterize_value(*knot, domain_start, domain_end, 0.0, 1.0)?;
                let numerator = c * normalized;
                let denominator = numerator + (1.0 - normalized);
                let mapped = numerator / denominator;
                require_finite([mapped], "NURBS endpoint-weight Möbius knot")?;
                Ok(mapped)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let knots = normalized_knots
            .iter()
            .map(|knot| reparameterize_value(*knot, 0.0, 1.0, domain_start, domain_end))
            .collect::<Result<Vec<_>, _>>()?;
        let end_log = end_weight.abs().ln();
        let controls = curve
            .control_points
            .iter()
            .enumerate()
            .map(|(index, control)| {
                let weight = if index == 0 || index + 1 == control_count {
                    1.0
                } else {
                    let mut log_magnitude = control.weight().abs().ln() - end_log;
                    for knot in &normalized_knots[index + 1..index + 1 + curve.degree] {
                        let factor = (1.0 - *knot).mul_add(c, *knot);
                        if !factor.is_finite() || factor <= 0.0 {
                            return Err(GeometryError::NonFinite {
                                context: "NURBS endpoint-weight Möbius control factor",
                            });
                        }
                        log_magnitude += factor.ln();
                    }
                    let sign =
                        if control.weight().is_sign_positive() == end_weight.is_sign_positive() {
                            1.0
                        } else {
                            -1.0
                        };
                    sign * log_magnitude.exp()
                };
                WeightedPoint3::try_new(control.point(), weight)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::try_new_rational(curve.degree, controls, knots)?.with_opennurbs_outer_knots())
    }

    /// Extracts an exact subcurve in Rhino's piecewise-Bezier trim form.
    ///
    /// Retained interior knots are raised to degree multiplicity. The first
    /// and last Bezier spans are then independently projectively
    /// reparameterized so the outer weights are one without shifting an
    /// interior knot or changing the curve locus.
    pub fn try_trimmed_with_normalized_end_weights(
        &self,
        interval: RangeInclusive<Real>,
    ) -> Result<Self, GeometryError> {
        let start = *interval.start();
        let end = *interval.end();
        let domain = self.domain();
        if !start.is_finite()
            || !end.is_finite()
            || start >= end
            || start < *domain.start()
            || end > *domain.end()
        {
            return Err(GeometryError::InvalidCurveTrimInterval);
        }
        let mut interior_knots = self
            .knots
            .iter()
            .copied()
            .filter(|knot| *knot > start && *knot < end)
            .collect::<Vec<_>>();
        interior_knots.dedup();

        let mut refined = self.clone();
        for knot in interior_knots {
            refined = refined.try_insert_knot(knot, self.degree)?;
        }
        let trimmed = refined.try_trimmed(interval)?;
        let span_count = trimmed.spans().count();
        let expected_control_count = span_count
            .checked_mul(trimmed.degree)
            .and_then(|count| count.checked_add(1))
            .ok_or(GeometryError::InvalidControlNet {
                context: "piecewise-Bezier trim control count overflowed",
            })?;
        if span_count == 0 || trimmed.control_points.len() != expected_control_count {
            return Err(GeometryError::InvalidControlNet {
                context: "trimmed curve must have degree-multiplicity interior knots",
            });
        }

        let weight_sign = trimmed.control_points[0].weight().is_sign_positive();
        if trimmed
            .control_points
            .iter()
            .any(|control| control.weight().is_sign_positive() != weight_sign)
        {
            return Err(GeometryError::InvalidControlNet {
                context: "piecewise-Bezier trim weights must have one sign",
            });
        }
        let sign_scale = if weight_sign { 1.0 } else { -1.0 };
        let mut controls = trimmed
            .control_points
            .iter()
            .map(|control| WeightedPoint3::try_new(control.point(), control.weight() * sign_scale))
            .collect::<Result<Vec<_>, _>>()?;

        if span_count == 1 {
            change_bezier_end_weights(&mut controls, 1.0, 1.0)?;
        } else {
            let first_end_weight = controls[trimmed.degree].weight();
            change_bezier_end_weights(&mut controls[..=trimmed.degree], 1.0, first_end_weight)?;
            let last_start = (span_count - 1) * trimmed.degree;
            let last_start_weight = controls[last_start].weight();
            change_bezier_end_weights(&mut controls[last_start..], last_start_weight, 1.0)?;
        }
        Self::try_new_rational(trimmed.degree, controls, trimmed.knots)
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
        // Keep distance comparisons and tangent residuals in one local frame.
        // Restoring a large world origin before subtracting the target can
        // quantize the objective and stop refinement before stationarity.
        let origin = self.control_points[0].point;
        if origin.to_array() != [0.0; 3] {
            let offset = Vector3::try_new(-origin.x(), -origin.y(), -origin.z())?;
            if let (Ok(local), Ok(target)) = (
                self.transformed(AffineTransform3::from_translation(offset)),
                target.translated(offset),
            ) {
                return local.closest_parameter_in_frame(target, tolerance);
            }
        }
        self.closest_parameter_in_frame(target, tolerance)
    }

    fn closest_parameter_in_frame(
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

    /// Finds finite intersections with another NURBS curve.
    ///
    /// Results are ordered by this curve's parameter, then the other curve's
    /// parameter. Collinear overlaps contribute their two boundary points.
    pub fn intersections_with_curve(
        &self,
        other: &Self,
        tolerance: Tolerance,
    ) -> Result<Vec<CurveCurveIntersection>, GeometryError> {
        let distance_tolerance = curve_pair_distance_tolerance(self, other, tolerance);
        let mut intersections = Vec::new();
        for event in self.intersection_events_with_curve(other, tolerance)? {
            match event {
                CurveCurveIntersectionEvent::Point(intersection) => {
                    push_unique_curve_intersection(
                        &mut intersections,
                        intersection,
                        distance_tolerance,
                    );
                }
                CurveCurveIntersectionEvent::Overlap(overlap) => {
                    for intersection in [overlap.start, overlap.end] {
                        push_unique_curve_intersection(
                            &mut intersections,
                            intersection,
                            distance_tolerance,
                        );
                    }
                }
            }
        }
        intersections.sort_by(compare_curve_intersections);
        Ok(intersections)
    }

    /// Finds distinct point contacts and finite shared intervals with another
    /// NURBS curve.
    ///
    /// Events are ordered by this curve's parameter. An overlap is returned
    /// once rather than also contributing point events at its boundaries.
    pub fn intersection_events_with_curve(
        &self,
        other: &Self,
        tolerance: Tolerance,
    ) -> Result<Vec<CurveCurveIntersectionEvent>, GeometryError> {
        const MAX_NODE_PAIRS: usize = 500_000;
        const MAX_DEPTH: u8 = 56;

        let coordinate_scale = self
            .control_points
            .iter()
            .chain(&other.control_points)
            .flat_map(|control| control.point.to_array())
            .fold(1.0_f64, |scale, coordinate| scale.max(coordinate.abs()));
        let distance_tolerance = tolerance
            .absolute()
            .max(tolerance.relative() * coordinate_scale);
        let refinement_tolerance =
            (distance_tolerance * 1.0e-4).max(Real::EPSILON * coordinate_scale * 64.0);
        let leaf_size = distance_tolerance * 2.0;
        let tangent_probe_size = (distance_tolerance * coordinate_scale).sqrt() * 2.0;
        let mut stack = Vec::new();
        let mut intersections = Vec::new();
        let mut overlaps = Vec::new();
        for first_span in self.spans() {
            let first =
                CurveIntersectionNode::try_new(self.try_trimmed(first_span.0..=first_span.1)?, 0)?;
            for second_span in other.spans() {
                let second = CurveIntersectionNode::try_new(
                    other.try_trimmed(second_span.0..=second_span.1)?,
                    0,
                )?;
                if first.convex_hull_bounds
                    && second.convex_hull_bounds
                    && (!bounding_boxes_overlap(first.bounds, second.bounds, distance_tolerance)
                        || !curve_control_hulls_overlap_on_local_axes(
                            &first.curve,
                            &second.curve,
                            distance_tolerance,
                        )?)
                {
                    continue;
                }
                if curve_nodes_are_certifiably_disjoint(&first, &second, distance_tolerance)? {
                    continue;
                }
                let overlap =
                    curve_span_overlap(&first.curve, &second.curve, distance_tolerance, tolerance)?;
                if let Some(overlap) = overlap {
                    push_unique_curve_overlap(&mut overlaps, overlap, distance_tolerance);
                } else {
                    let tangencies = initial_curve_curve_intersections(
                        self,
                        other,
                        &second.curve,
                        [first_span.0, first_span.1],
                        [second_span.0, second_span.1],
                        refinement_tolerance,
                        distance_tolerance,
                        tolerance,
                    )?;
                    for &intersection in &tangencies {
                        push_unique_curve_intersection(
                            &mut intersections,
                            intersection,
                            distance_tolerance,
                        );
                    }
                    let first_parameters = partition_curve_span_at(
                        first_span.0,
                        first_span.1,
                        tangencies
                            .iter()
                            .map(|intersection| intersection.first_parameter),
                    );
                    let second_parameters = partition_curve_span_at(
                        second_span.0,
                        second_span.1,
                        tangencies
                            .iter()
                            .map(|intersection| intersection.second_parameter),
                    );
                    for first_interval in first_parameters.windows(2) {
                        let first_piece = CurveIntersectionNode::try_new(
                            self.try_trimmed(first_interval[0]..=first_interval[1])?,
                            0,
                        )?;
                        for second_interval in second_parameters.windows(2) {
                            stack.push((
                                first_piece.clone(),
                                CurveIntersectionNode::try_new(
                                    other.try_trimmed(second_interval[0]..=second_interval[1])?,
                                    0,
                                )?,
                            ));
                        }
                    }
                }
            }
        }

        let mut processed = 0_usize;
        while let Some((first, second)) = stack.pop() {
            processed += 1;
            if processed > MAX_NODE_PAIRS {
                return Err(GeometryError::CurveIntersectionDidNotConverge);
            }
            if first.convex_hull_bounds
                && second.convex_hull_bounds
                && (!bounding_boxes_overlap(first.bounds, second.bounds, distance_tolerance)
                    || !curve_control_hulls_overlap_on_local_axes(
                        &first.curve,
                        &second.curve,
                        distance_tolerance,
                    )?)
            {
                continue;
            }

            let first_size = first.spatial_size()?;
            let second_size = second.spatial_size()?;
            let leaf = (first.convex_hull_bounds
                && second.convex_hull_bounds
                && first_size <= leaf_size
                && second_size <= leaf_size)
                || (first.depth >= MAX_DEPTH && second.depth >= MAX_DEPTH);
            let tangent_probe = first.convex_hull_bounds
                && second.convex_hull_bounds
                && first_size <= tangent_probe_size
                && second_size <= tangent_probe_size;
            if leaf || tangent_probe {
                let first_domain = first.curve.domain();
                let second_domain = second.curve.domain();
                let (first_fraction, second_fraction) = closest_segment_fractions(
                    first.curve.evaluate(*first_domain.start())?,
                    first.curve.evaluate(*first_domain.end())?,
                    second.curve.evaluate(*second_domain.start())?,
                    second.curve.evaluate(*second_domain.end())?,
                )?;
                let first_seed = interpolate_parameter(
                    *first_domain.start(),
                    *first_domain.end(),
                    first_fraction,
                );
                let second_seed = interpolate_parameter(
                    *second_domain.start(),
                    *second_domain.end(),
                    second_fraction,
                );
                let mut intersection = refine_curve_curve_intersection(
                    self,
                    other,
                    first_seed,
                    second_seed,
                    [*first_domain.start(), *first_domain.end()],
                    [*second_domain.start(), *second_domain.end()],
                    refinement_tolerance,
                )?;
                if intersection.is_none() && tangent_probe {
                    intersection = refine_tangent_curve_curve_intersection(
                        self,
                        other,
                        &second.curve,
                        [*first_domain.start(), *first_domain.end()],
                        [*second_domain.start(), *second_domain.end()],
                        refinement_tolerance,
                        tolerance,
                    )?;
                }
                if let Some(intersection) = intersection {
                    push_unique_curve_intersection(
                        &mut intersections,
                        intersection,
                        distance_tolerance,
                    );
                    continue;
                }
                if leaf {
                    continue;
                }
            }

            let split_first = !first.convex_hull_bounds
                || (second.convex_hull_bounds
                    && first.depth < MAX_DEPTH
                    && (first_size >= second_size || second.depth >= MAX_DEPTH));
            if split_first && first.depth < MAX_DEPTH {
                let [left, right] = first.split()?;
                stack.push((right, second.clone()));
                stack.push((left, second));
            } else if second.depth < MAX_DEPTH {
                let [left, right] = second.split()?;
                stack.push((first.clone(), right));
                stack.push((first, left));
            } else {
                return Err(GeometryError::CurveIntersectionDidNotConverge);
            }
        }
        overlaps.sort_by(compare_curve_overlaps);
        let overlaps = merge_adjacent_curve_overlaps(overlaps, distance_tolerance);
        intersections.retain(|intersection| {
            !overlaps
                .iter()
                .any(|overlap| curve_overlap_contains_intersection(*overlap, *intersection))
        });
        intersections.sort_by(compare_curve_intersections);

        let mut events = intersections
            .into_iter()
            .map(CurveCurveIntersectionEvent::Point)
            .chain(
                overlaps
                    .into_iter()
                    .map(CurveCurveIntersectionEvent::Overlap),
            )
            .collect::<Vec<_>>();
        events.sort_by(|left, right| {
            curve_intersection_event_parameter(*left)
                .total_cmp(&curve_intersection_event_parameter(*right))
        });
        Ok(events)
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
        require_finite([parameter], "curve seam parameter")?;
        if !self.is_closed()? {
            return Err(GeometryError::CurveSeamMustBeClosed);
        }
        let domain = self.domain();
        if !domain.contains(&parameter) {
            let wrapped = crate::parameter::wrapped_parameter(parameter, &domain)?;
            // OpenNURBS' periodic seam mover receives the original parameter
            // and falls back to clamped split/append outside the old interval.
            let relocated = if wrapped > *domain.start() && wrapped < *domain.end() {
                self.change_non_periodic_seam(
                    wrapped,
                    *domain.start(),
                    *domain.end(),
                    self.is_periodic(),
                )?
            } else {
                self.clone()
            };
            return relocated.try_reparameterized(
                parameter..=crate::parameter::shifted_parameter(parameter, &domain)?,
            );
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
        let join = right.control_points[right.control_points.len() - 1].point;
        right.try_append_clamped_at_join(&shifted_left, join)
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
        let sine = incoming
            .as_vector()
            .cross(outgoing.as_vector())?
            .length()?
            .clamp(0.0, 1.0);
        Ok(sine.atan2(cosine))
    }

    pub(crate) fn full_multiplicity_kink_parameters(
        &self,
        tolerance: Tolerance,
    ) -> Result<Vec<Real>, GeometryError> {
        let mut kinks = Vec::new();
        for (knot, multiplicity) in self.interior_knot_groups() {
            if multiplicity == self.degree && self.kink_angle_at(knot)? > tolerance.angular() {
                kinks.push(knot);
            }
        }
        Ok(kinks)
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

    /// Splits at one or more distinct parameters strictly inside the active
    /// domain. Open-curve pieces follow the source domain; closed-curve pieces
    /// run cyclically between sorted parameters, crossing the original seam
    /// for the final piece.
    pub fn try_split_at_parameters(&self, parameters: &[Real]) -> Result<Vec<Self>, GeometryError> {
        if parameters.is_empty() {
            return Err(GeometryError::InvalidCurveSplitParameter);
        }
        let mut parameters = parameters.to_vec();
        parameters.sort_by(Real::total_cmp);
        let domain = self.domain();
        if parameters.iter().any(|parameter| {
            !parameter.is_finite() || *parameter <= *domain.start() || *parameter >= *domain.end()
        }) || parameters.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(GeometryError::InvalidCurveSplitParameter);
        }

        if self.is_closed()? {
            if let [parameter] = parameters.as_slice() {
                return Ok(vec![
                    self.try_closed_subcurve_across_seam(*parameter, *parameter)?,
                ]);
            }
            let mut pieces = Vec::with_capacity(parameters.len());
            for pair in parameters.windows(2) {
                pieces.push(self.try_subcurve(pair[0], pair[1])?);
            }
            pieces.push(
                self.try_subcurve(
                    *parameters
                        .last()
                        .expect("closed curve split parameters are nonempty"),
                    parameters[0],
                )?,
            );
            return Ok(pieces);
        }

        let mut pieces = Vec::with_capacity(parameters.len() + 1);
        let mut remainder = self.clone();
        for parameter in parameters {
            let (left, right) = remainder.try_split(parameter)?;
            pieces.push(left);
            remainder = right;
        }
        pieces.push(remainder);
        Ok(pieces)
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

    /// Extends an open curve to either or both requested parameter bounds.
    ///
    /// Each extended end is first clamped at its current endpoint and then
    /// extrapolated with the endpoint NURBS span. Bounds that fall inside the
    /// current domain are ignored, matching OpenNURBS' natural extension
    /// behavior. At least one bound must extend the current domain.
    pub fn try_extended_to(&self, interval: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        self.try_extended_to_impl(interval, true)
    }

    pub(crate) fn try_extended_control_curve_to(
        &self,
        interval: RangeInclusive<Real>,
    ) -> Result<Self, GeometryError> {
        self.try_extended_to_impl(interval, false)
    }

    pub(crate) fn try_extended_linearly_control_curve_to(
        &self,
        interval: RangeInclusive<Real>,
    ) -> Result<Self, GeometryError> {
        let requested_start = *interval.start();
        let requested_end = *interval.end();
        if !requested_start.is_finite()
            || !requested_end.is_finite()
            || requested_start >= requested_end
        {
            return Err(GeometryError::InvalidCurveExtensionInterval);
        }

        let domain = self.domain();
        let original_start = *domain.start();
        let original_end = *domain.end();
        let extend_start = requested_start < original_start;
        let extend_end = requested_end > original_end;
        if !extend_start && !extend_end {
            return Err(GeometryError::InvalidCurveExtensionInterval);
        }

        let mut result = self.clone();
        if extend_start {
            result = result.clamped_at_start(original_start)?;
            let adjacent_span = result
                .spans()
                .next()
                .expect("a validated NURBS curve has an active span")
                .1
                - original_start;
            let parameter_delta = original_start - requested_start;
            let first = result.control_points[0];
            let next = result.control_points[1];
            let mut controls = Vec::with_capacity(result.control_points.len() + result.degree);
            for index in 0..result.degree {
                let factor = (result.degree - index) as Real * parameter_delta / adjacent_span;
                controls.push(blend_weighted_control_points_unbounded(
                    first, next, -factor,
                )?);
            }
            controls.extend_from_slice(&result.control_points);
            let mut knots = Vec::with_capacity(controls.len() + result.degree + 1);
            knots.resize(result.degree + 1, requested_start);
            knots.extend_from_slice(&result.knots[1..]);
            result = Self::try_new_rational(result.degree, controls, knots)?;
        }
        if extend_end {
            result = result.clamped_at_end(original_end)?;
            let adjacent_span = original_end
                - result
                    .spans()
                    .last()
                    .expect("a validated NURBS curve has an active span")
                    .0;
            let parameter_delta = requested_end - original_end;
            let last = result.control_points.len() - 1;
            let previous = result.control_points[last - 1];
            let endpoint = result.control_points[last];
            let mut controls = result.control_points;
            controls.reserve_exact(result.degree);
            for index in 1..=result.degree {
                let factor = index as Real * parameter_delta / adjacent_span;
                controls.push(blend_weighted_control_points_unbounded(
                    previous,
                    endpoint,
                    1.0 + factor,
                )?);
            }
            let mut knots = result.knots;
            knots.pop();
            knots.resize(knots.len() + result.degree + 1, requested_end);
            result = Self::try_new_rational(result.degree, controls, knots)?;
        }
        Ok(result)
    }

    fn try_extended_to_impl(
        &self,
        interval: RangeInclusive<Real>,
        require_open: bool,
    ) -> Result<Self, GeometryError> {
        let requested_start = *interval.start();
        let requested_end = *interval.end();
        if !requested_start.is_finite()
            || !requested_end.is_finite()
            || requested_start >= requested_end
        {
            return Err(GeometryError::InvalidCurveExtensionInterval);
        }
        if require_open && self.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen);
        }

        let domain = self.domain();
        let original_start = *domain.start();
        let original_end = *domain.end();
        let extend_start = requested_start < original_start;
        let extend_end = requested_end > original_end;
        if !extend_start && !extend_end {
            return Err(GeometryError::InvalidCurveExtensionInterval);
        }

        let mut result = self.clone();
        if extend_start {
            result = result.clamped_at_start(original_start)?;
            let (_, controls) =
                result.de_boor_side_controls_unbounded(result.degree, requested_start)?;
            let mut all_controls = result.control_points;
            all_controls[..=result.degree].copy_from_slice(&controls);
            let mut knots = result.knots;
            knots[..=result.degree].fill(requested_start);
            result = Self::try_new_rational(result.degree, all_controls, knots)?;
        }
        if extend_end {
            result = result.clamped_at_end(original_end)?;
            let final_span = result.control_points.len() - 1;
            let (controls, _) =
                result.de_boor_side_controls_unbounded(final_span, requested_end)?;
            let first_control = result.control_points.len() - result.degree - 1;
            let mut all_controls = result.control_points;
            all_controls[first_control..].copy_from_slice(&controls);
            let mut knots = result.knots;
            let first_knot = knots.len() - result.degree - 1;
            knots[first_knot..].fill(requested_end);
            result = Self::try_new_rational(result.degree, all_controls, knots)?;
        }
        Ok(result)
    }

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
        let targets = self.curve_boundary_extension_targets(side, style, boundaries, tolerance)?;
        match targets.style {
            CurveExtensionStyle::Smooth => self.smooth_boundary_extension(&targets, false),
            CurveExtensionStyle::Line => self
                .apply_boundary_extension_lengths(&targets, |curve, side, length| {
                    curve.try_merged_linearly_by_length(side, length, tolerance)
                }),
            CurveExtensionStyle::Arc => {
                self.apply_boundary_extension_lengths(&targets, |curve, side, length| {
                    curve.try_merged_circularly_by_length(side, length, tolerance)
                })
            }
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
        let targets = self.curve_boundary_extension_targets(side, style, boundaries, tolerance)?;
        match targets.style {
            CurveExtensionStyle::Smooth => self.smooth_boundary_extension(&targets, true),
            CurveExtensionStyle::Line => self
                .apply_boundary_extension_lengths(&targets, |curve, side, length| {
                    curve.try_joined_linearly_by_length(side, length, tolerance)
                }),
            CurveExtensionStyle::Arc => {
                self.apply_boundary_extension_lengths(&targets, |curve, side, length| {
                    curve.try_joined_circularly_by_length(side, length, tolerance)
                })
            }
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
        let targets = self.curve_boundary_extension_targets(side, style, boundaries, tolerance)?;
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
                    vec![self.extension_segment(
                        extension_side == CurveExtensionSide::Start,
                        target.parameter,
                    )?]
                }
                CurveExtensionStyle::Line => self.try_separate_linear_extensions_by_length(
                    extension_side,
                    target.length,
                    tolerance,
                )?,
                CurveExtensionStyle::Arc => self.try_separate_circular_extensions_by_length(
                    extension_side,
                    target.length,
                    tolerance,
                )?,
                CurveExtensionStyle::Natural => {
                    unreachable!("boundary extension resolves Natural before constructing targets")
                }
            };
            debug_assert_eq!(pieces.len(), 1);
            extensions.append(&mut pieces);
        }
        Ok(extensions)
    }

    fn curve_boundary_extension_targets(
        &self,
        side: CurveExtensionSide,
        style: CurveExtensionStyle,
        boundaries: &[CurveExtensionBoundary],
        tolerance: Tolerance,
    ) -> Result<CurveBoundaryExtensionTargets, GeometryError> {
        if boundaries.is_empty() {
            return Err(GeometryError::EmptyCurveExtensionBoundaries);
        }
        if self.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen);
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
            .then(|| self.curve_boundary_extension_target(true, style, boundaries, tolerance))
            .transpose()?
            .flatten();
        let end = matches!(side, CurveExtensionSide::End | CurveExtensionSide::Both)
            .then(|| self.curve_boundary_extension_target(false, style, boundaries, tolerance))
            .transpose()?
            .flatten();
        if (matches!(side, CurveExtensionSide::Start) && start.is_none())
            || (matches!(side, CurveExtensionSide::End) && end.is_none())
            || (matches!(side, CurveExtensionSide::Both) && start.is_none() && end.is_none())
        {
            return Err(GeometryError::CurveExtensionBoundaryNotFound);
        }
        Ok(CurveBoundaryExtensionTargets { style, start, end })
    }

    fn curve_boundary_extension_target(
        &self,
        at_start: bool,
        style: CurveExtensionStyle,
        boundaries: &[CurveExtensionBoundary],
        tolerance: Tolerance,
    ) -> Result<Option<CurveBoundaryExtensionTarget>, GeometryError> {
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
                let extension = self.extension_segment(at_start, outer_parameter)?;
                if let Some(hit) = extension
                    .nearest_boundary_intersection(boundaries, at_start, endpoint, tolerance)?
                {
                    let interval = if at_start {
                        hit.first_parameter..=endpoint_parameter
                    } else {
                        endpoint_parameter..=hit.first_parameter
                    };
                    let length = extension.try_trimmed(interval)?.length(tolerance)?;
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
        let length = extension.try_trimmed(interval)?.length(tolerance)?;
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
        for boundary in boundaries {
            let intersections = match boundary {
                CurveExtensionBoundary::Curve(curve) => {
                    self.intersections_with_curve(curve, tolerance)?
                }
                CurveExtensionBoundary::Surface(surface) => {
                    curve_surface_intersections(self, surface, tolerance)?
                        .into_iter()
                        .map(|intersection| CurveCurveIntersection {
                            first_parameter: intersection.curve_parameter,
                            second_parameter: intersection.u,
                            point: intersection.point,
                        })
                        .collect()
                }
                CurveExtensionBoundary::Brep(brep) => {
                    let mut intersections = Vec::new();
                    for edge in brep.edges() {
                        intersections
                            .extend(self.intersections_with_curve(edge.curve(), tolerance)?);
                    }
                    for face in brep.faces() {
                        for intersection in
                            curve_surface_intersections(self, face.surface(), tolerance)?
                        {
                            if face.contains_parameters(
                                intersection.u,
                                intersection.v,
                                tolerance,
                            )? {
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

    fn smooth_boundary_extension(
        &self,
        targets: &CurveBoundaryExtensionTargets,
        retain_seams: bool,
    ) -> Result<Self, GeometryError> {
        let domain = self.domain();
        if !retain_seams {
            return self.try_extended_to(
                targets
                    .start
                    .map_or(*domain.start(), |target| target.parameter)
                    ..=targets.end.map_or(*domain.end(), |target| target.parameter),
            );
        }
        let start_extension = targets
            .start
            .map(|target| self.extension_segment(true, target.parameter))
            .transpose()?;
        let end_extension = targets
            .end
            .map(|target| self.extension_segment(false, target.parameter))
            .transpose()?;
        let mut source = self.clone();
        if start_extension.is_some() {
            source = source.clamped_at_start(*domain.start())?;
        }
        if end_extension.is_some() {
            source = source.clamped_at_end(*domain.end())?;
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

    /// Smoothly extrapolates the selected end or ends by the requested arc
    /// length. `Both` applies the full length independently at each end.
    pub fn try_extended_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !length.is_finite() || length <= 0.0 {
            return Err(GeometryError::InvalidCurveExtensionLength);
        }
        if self.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen);
        }

        let domain = self.domain();
        let start = match side {
            CurveExtensionSide::Start | CurveExtensionSide::Both => {
                self.extension_parameter_by_length(true, length, tolerance)?
            }
            CurveExtensionSide::End => *domain.start(),
        };
        let end = match side {
            CurveExtensionSide::End | CurveExtensionSide::Both => {
                self.extension_parameter_by_length(false, length, tolerance)?
            }
            CurveExtensionSide::Start => *domain.end(),
        };
        self.try_extended_to(start..=end)
    }

    /// Applies Rhino's type-sensitive `Natural` merge behavior: line-like
    /// sources extend linearly, exact circular arcs retain their radius, and
    /// other curves use smooth NURBS extrapolation.
    pub fn try_merged_naturally_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        self.validate_length_extension(length)?;
        if self.degree == 1 || self.is_linear_at_zero_tolerance()? {
            return self.try_merged_linearly_by_length(side, length, tolerance);
        }
        if self.try_canonical_circular_arc(tolerance)?.is_some() {
            return self.try_merged_circularly_by_length(side, length, tolerance);
        }
        self.try_extended_by_length(side, length, tolerance)
    }

    /// Extends an open curve with an exact degree-matched straight tangent
    /// span of the requested model-space length. `Both` applies the full
    /// length independently at each end, matching Rhino's line-style curve
    /// extension.
    pub fn try_extended_linearly_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !length.is_finite() || length <= 0.0 {
            return Err(GeometryError::InvalidCurveExtensionLength);
        }
        if self.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen);
        }

        let mut result = self.clone();
        if matches!(side, CurveExtensionSide::Start | CurveExtensionSide::Both) {
            result = result.extended_linearly_at_start(length, tolerance)?;
        }
        if matches!(side, CurveExtensionSide::End | CurveExtensionSide::Both) {
            result = result.extended_linearly_at_end(length, tolerance)?;
        }
        Ok(result)
    }

    /// Applies Rhino's `Join=Merge` cleanup to a tangent-line extension.
    /// Degree-one curves merge into their terminal line span, geometrically
    /// linear higher-degree curves collapse to one line, and other curves
    /// retain the degree-matched appended span.
    pub fn try_merged_linearly_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if self.degree == 1 {
            return self.try_extended_by_length(side, length, tolerance);
        }
        if !self.is_linear_at_zero_tolerance()? {
            return self.try_extended_linearly_by_length(side, length, tolerance);
        }
        if !length.is_finite() || length <= 0.0 {
            return Err(GeometryError::InvalidCurveExtensionLength);
        }
        if self.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen);
        }

        let domain = self.domain();
        let original_start = *domain.start();
        let original_end = *domain.end();
        let start_derivative = self.derivative_at(original_start)?;
        let end_derivative = self.derivative_at(original_end)?;
        let start_speed = start_derivative.length()?;
        let end_speed = end_derivative.length()?;
        let start_tangent = start_derivative.normalized(tolerance)?;
        let end_tangent = end_derivative.normalized(tolerance)?;
        let mut start = self.evaluate(original_start)?;
        let mut end = self.evaluate(original_end)?;
        let mut domain_start = original_start;
        let mut domain_end = original_end;
        if matches!(side, CurveExtensionSide::Start | CurveExtensionSide::Both) {
            domain_start -= length / start_speed;
            start = start.translated(start_tangent.as_vector().scaled(-length)?)?;
        }
        if matches!(side, CurveExtensionSide::End | CurveExtensionSide::Both) {
            domain_end += length / end_speed;
            end = end.translated(end_tangent.as_vector().scaled(length)?)?;
        }
        require_finite(
            [domain_start, domain_end],
            "merged linear curve extension domain",
        )?;
        if domain_start >= original_start && domain_end <= original_end {
            return Err(GeometryError::CurveExtensionLengthDidNotConverge);
        }
        Self::try_new(
            1,
            vec![start, end],
            vec![domain_start, domain_start, domain_end, domain_end],
        )
    }

    /// Extends with the endpoint's exact osculating arc. A zero-curvature end
    /// falls back to a degree-matched tangent-line span, and sweeps beyond one
    /// revolution cap at a full circle, as Rhino does.
    pub fn try_extended_circularly_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        self.validate_length_extension(length)?;

        let domain = self.domain();
        let start_extension =
            if matches!(side, CurveExtensionSide::Start | CurveExtensionSide::Both) {
                Some(self.circular_extension_piece(true, length, tolerance, true)?)
            } else {
                None
            };
        let end_extension = if matches!(side, CurveExtensionSide::End | CurveExtensionSide::Both) {
            Some(self.circular_extension_piece(false, length, tolerance, true)?)
        } else {
            None
        };
        let mut source = self.clone();
        if start_extension.is_some() {
            source = source.clamped_at_start(*domain.start())?;
        }
        if end_extension.is_some() {
            source = source.clamped_at_end(*domain.end())?;
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

    /// Applies Rhino's `Join=Merge` cleanup to an osculating-arc extension.
    /// Lines merge linearly and a same-circle source becomes one canonical arc;
    /// other sources retain the exact joined arc boundary.
    pub fn try_merged_circularly_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if self.degree == 1 || self.is_linear_at_zero_tolerance()? {
            return self.try_merged_linearly_by_length(side, length, tolerance);
        }
        let extended = self.try_extended_circularly_by_length(side, length, tolerance)?;
        Ok(extended
            .try_canonical_circular_arc(tolerance)?
            .unwrap_or(extended))
    }

    /// Joins osculating-arc extensions while retaining explicit source
    /// boundaries. Degree-one sources use Rhino's unit-span polyline form.
    pub fn try_joined_circularly_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if self.degree == 1 {
            self.try_joined_linearly_by_length(side, length, tolerance)
        } else {
            self.try_extended_circularly_by_length(side, length, tolerance)
        }
    }

    /// Creates independent osculating-arc extension pieces without changing
    /// the source. Degree-one sources produce unit-domain tangent lines.
    pub fn try_separate_circular_extensions_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Vec<Self>, GeometryError> {
        if self.degree == 1 {
            return self.try_separate_linear_extensions_by_length(side, length, tolerance);
        }
        self.validate_length_extension(length)?;
        let mut extensions = Vec::with_capacity(if side == CurveExtensionSide::Both {
            2
        } else {
            1
        });
        if matches!(side, CurveExtensionSide::Start | CurveExtensionSide::Both) {
            let mut extension = self.circular_extension_piece(true, length, tolerance, false)?;
            if extension.degree == 1 {
                extension = extension.try_reparameterized(0.0..=1.0)?;
            }
            extensions.push(extension);
        }
        if matches!(side, CurveExtensionSide::End | CurveExtensionSide::Both) {
            let mut extension = self.circular_extension_piece(false, length, tolerance, false)?;
            if extension.degree == 1 {
                extension = extension.try_reparameterized(0.0..=1.0)?;
            }
            extensions.push(extension);
        }
        Ok(extensions)
    }

    /// Joins tangent-line extensions while retaining explicit segment
    /// boundaries, matching Rhino's `Join=Yes` command behavior.
    pub fn try_joined_linearly_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if self.degree != 1 {
            return self.try_extended_linearly_by_length(side, length, tolerance);
        }
        if !length.is_finite() || length <= 0.0 {
            return Err(GeometryError::InvalidCurveExtensionLength);
        }
        if self.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen);
        }

        let spans = self.spans().collect::<Vec<_>>();
        let mut points = Vec::with_capacity(
            spans.len()
                + 1
                + usize::from(matches!(
                    side,
                    CurveExtensionSide::Start | CurveExtensionSide::Both
                ))
                + usize::from(matches!(
                    side,
                    CurveExtensionSide::End | CurveExtensionSide::Both
                )),
        );
        let domain = self.domain();
        if matches!(side, CurveExtensionSide::Start | CurveExtensionSide::Both) {
            let parameter = *domain.start();
            let endpoint = self.evaluate(parameter)?;
            let tangent = self.derivative_at(parameter)?.normalized(tolerance)?;
            points.push(endpoint.translated(tangent.as_vector().scaled(-length)?)?);
        }
        for (start, _) in &spans {
            points.push(self.evaluate(*start)?);
        }
        let end_parameter = *domain.end();
        let endpoint = self.evaluate(end_parameter)?;
        points.push(endpoint);
        if matches!(side, CurveExtensionSide::End | CurveExtensionSide::Both) {
            let tangent = self.derivative_at(end_parameter)?.normalized(tolerance)?;
            points.push(endpoint.translated(tangent.as_vector().scaled(length)?)?);
        }

        let segment_count = points.len() - 1;
        let mut knots = Vec::with_capacity(points.len() + 2);
        knots.push(0.0);
        knots.extend((0..=segment_count).map(|index| index as Real));
        knots.push(segment_count as Real);
        Self::try_new(1, points, knots)
    }

    /// Joins natural extension pieces to the unchanged source with explicit
    /// full-multiplicity seams, matching Rhino's `Join=Yes` curve result.
    pub fn try_joined_naturally_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        self.validate_length_extension(length)?;
        if self.degree == 1 || self.is_linear_at_zero_tolerance()? {
            return self.try_joined_linearly_by_length(side, length, tolerance);
        }
        if self.try_canonical_circular_arc(tolerance)?.is_some() {
            return self.try_joined_circularly_by_length(side, length, tolerance);
        }
        self.try_joined_smoothly_by_length(side, length, tolerance)
    }

    /// Joins smooth NURBS extrapolation pieces to the unchanged source with
    /// explicit full-multiplicity seams.
    pub fn try_joined_smoothly_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if self.degree == 1 {
            return self.try_joined_linearly_by_length(side, length, tolerance);
        }
        if !length.is_finite() || length <= 0.0 {
            return Err(GeometryError::InvalidCurveExtensionLength);
        }
        if self.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen);
        }

        let domain = self.domain();
        let start_extension =
            if matches!(side, CurveExtensionSide::Start | CurveExtensionSide::Both) {
                let parameter = self.extension_parameter_by_length(true, length, tolerance)?;
                Some(self.extension_segment(true, parameter)?)
            } else {
                None
            };
        let end_extension = if matches!(side, CurveExtensionSide::End | CurveExtensionSide::Both) {
            let parameter = self.extension_parameter_by_length(false, length, tolerance)?;
            Some(self.extension_segment(false, parameter)?)
        } else {
            None
        };
        let mut source = self.clone();
        if start_extension.is_some() {
            source = source.clamped_at_start(*domain.start())?;
        }
        if end_extension.is_some() {
            source = source.clamped_at_end(*domain.end())?;
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

    /// Creates independent degree-one tangent-line extensions without
    /// changing this curve. Each output uses Rhino command geometry's unit
    /// domain, and `Both` returns the start extension followed by the end
    /// extension.
    pub fn try_separate_linear_extensions_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Vec<Self>, GeometryError> {
        if !length.is_finite() || length <= 0.0 {
            return Err(GeometryError::InvalidCurveExtensionLength);
        }
        if self.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen);
        }

        let domain = self.domain();
        let mut extensions = Vec::with_capacity(if side == CurveExtensionSide::Both {
            2
        } else {
            1
        });
        if matches!(side, CurveExtensionSide::Start | CurveExtensionSide::Both) {
            let parameter = *domain.start();
            let endpoint = self.evaluate(parameter)?;
            let tangent = self.derivative_at(parameter)?.normalized(tolerance)?;
            let outer = endpoint.translated(tangent.as_vector().scaled(-length)?)?;
            extensions.push(Self::try_new(
                1,
                vec![outer, endpoint],
                vec![0.0, 0.0, 1.0, 1.0],
            )?);
        }
        if matches!(side, CurveExtensionSide::End | CurveExtensionSide::Both) {
            let parameter = *domain.end();
            let endpoint = self.evaluate(parameter)?;
            let tangent = self.derivative_at(parameter)?.normalized(tolerance)?;
            let outer = endpoint.translated(tangent.as_vector().scaled(length)?)?;
            extensions.push(Self::try_new(
                1,
                vec![endpoint, outer],
                vec![0.0, 0.0, 1.0, 1.0],
            )?);
        }
        Ok(extensions)
    }

    /// Creates independent natural-extension pieces without changing this
    /// curve. Degree-one inputs use Rhino's standalone line representation;
    /// other inputs retain the natural extension's source parameter values.
    pub fn try_separate_natural_extensions_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Vec<Self>, GeometryError> {
        self.validate_length_extension(length)?;
        if self.degree == 1 || self.is_linear_at_zero_tolerance()? {
            return self.try_separate_linear_extensions_by_length(side, length, tolerance);
        }
        if self.try_canonical_circular_arc(tolerance)?.is_some() {
            return self.try_separate_circular_extensions_by_length(side, length, tolerance);
        }
        self.try_separate_smooth_extensions_by_length(side, length, tolerance)
    }

    /// Creates independent smooth NURBS extrapolation pieces without changing
    /// the source. Degree-one inputs use standalone line representation.
    pub fn try_separate_smooth_extensions_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Vec<Self>, GeometryError> {
        if self.degree == 1 {
            return self.try_separate_linear_extensions_by_length(side, length, tolerance);
        }
        if !length.is_finite() || length <= 0.0 {
            return Err(GeometryError::InvalidCurveExtensionLength);
        }
        if self.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen);
        }

        let mut extensions = Vec::with_capacity(if side == CurveExtensionSide::Both {
            2
        } else {
            1
        });
        if matches!(side, CurveExtensionSide::Start | CurveExtensionSide::Both) {
            let parameter = self.extension_parameter_by_length(true, length, tolerance)?;
            extensions.push(self.extension_segment(true, parameter)?);
        }
        if matches!(side, CurveExtensionSide::End | CurveExtensionSide::Both) {
            let parameter = self.extension_parameter_by_length(false, length, tolerance)?;
            extensions.push(self.extension_segment(false, parameter)?);
        }
        Ok(extensions)
    }

    fn extended_linearly_at_start(
        &self,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let endpoint_parameter = *self.domain().start();
        let derivative = self.derivative_at(endpoint_parameter)?;
        let speed = derivative.length()?;
        let tangent = derivative.normalized(tolerance)?;
        let parameter = endpoint_parameter - length / speed;
        require_finite([parameter], "linear curve extension parameter")?;
        if parameter == endpoint_parameter {
            return Err(GeometryError::CurveExtensionLengthDidNotConverge);
        }

        let source = self.clamped_at_start(endpoint_parameter)?;
        let endpoint = source.control_points[0];
        let degree = source.degree as Real;
        let mut controls = Vec::with_capacity(source.control_points.len() + source.degree);
        for index in 0..source.degree {
            let fraction = (source.degree - index) as Real / degree;
            let point = endpoint
                .point
                .translated(tangent.as_vector().scaled(-length * fraction)?)?;
            controls.push(WeightedPoint3::try_new(point, 1.0)?);
        }
        controls.extend(
            source
                .control_points
                .iter()
                .map(|control| {
                    WeightedPoint3::try_new(control.point, control.weight / endpoint.weight)
                })
                .collect::<Result<Vec<_>, _>>()?,
        );

        let mut knots = Vec::with_capacity(controls.len() + source.degree + 1);
        knots.resize(source.degree + 1, parameter);
        knots.extend_from_slice(&source.knots[1..]);
        Self::try_new_rational(source.degree, controls, knots)
    }

    fn extended_linearly_at_end(
        &self,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let endpoint_parameter = *self.domain().end();
        let derivative = self.derivative_at(endpoint_parameter)?;
        let speed = derivative.length()?;
        let tangent = derivative.normalized(tolerance)?;
        let parameter = endpoint_parameter + length / speed;
        require_finite([parameter], "linear curve extension parameter")?;
        if parameter == endpoint_parameter {
            return Err(GeometryError::CurveExtensionLengthDidNotConverge);
        }

        let source = self.clamped_at_end(endpoint_parameter)?;
        let endpoint = source.control_points[source.control_points.len() - 1];
        let degree = source.degree as Real;
        let mut controls = source.control_points;
        controls.reserve_exact(source.degree);
        for index in 1..=source.degree {
            let fraction = index as Real / degree;
            let point = endpoint
                .point
                .translated(tangent.as_vector().scaled(length * fraction)?)?;
            controls.push(WeightedPoint3::try_new(point, endpoint.weight)?);
        }

        let mut knots = source.knots;
        knots.pop();
        knots.resize(knots.len() + source.degree + 1, parameter);
        Self::try_new_rational(source.degree, controls, knots)
    }

    fn extension_parameter_by_length(
        &self,
        at_start: bool,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Real, GeometryError> {
        let domain = self.domain();
        let endpoint = if at_start {
            *domain.start()
        } else {
            *domain.end()
        };
        let adjacent_span = if at_start {
            self.spans()
                .next()
                .expect("a validated NURBS curve has an active span")
                .1
                - endpoint
        } else {
            endpoint
                - self
                    .spans()
                    .last()
                    .expect("a validated NURBS curve has an active span")
                    .0
        };
        debug_assert!(adjacent_span.is_finite() && adjacent_span > 0.0);
        let endpoint_speed = self.derivative_at(endpoint)?.length()?;
        let estimated_step = if endpoint_speed > 0.0 {
            length / endpoint_speed
        } else {
            adjacent_span
        };
        let minimum_step = adjacent_span * 1.0e-6;
        let mut step = if estimated_step.is_finite() && estimated_step > 0.0 {
            estimated_step.max(minimum_step)
        } else {
            adjacent_span
        };

        for _ in 0..128 {
            let parameter = if at_start {
                endpoint - step
            } else {
                endpoint + step
            };
            if !parameter.is_finite() || parameter == endpoint {
                return Err(GeometryError::CurveExtensionLengthDidNotConverge);
            }
            let extension = self.extension_segment(at_start, parameter)?;
            let sampler = ArcLengthSampler::try_new(CurveRef::NurbsCurve(&extension), tolerance)?;
            let extension_length = sampler.total_length();
            if extension_length >= length {
                let distance = if at_start {
                    extension_length - length
                } else {
                    length
                };
                return sampler.parameter_at_distance(distance);
            }
            step *= 2.0;
            if !step.is_finite() {
                return Err(GeometryError::CurveExtensionLengthDidNotConverge);
            }
        }
        Err(GeometryError::CurveExtensionLengthDidNotConverge)
    }

    fn extension_segment(&self, at_start: bool, parameter: Real) -> Result<Self, GeometryError> {
        let domain = self.domain();
        let start = *domain.start();
        let end = *domain.end();
        if at_start {
            self.try_extended_to(parameter..=end)?
                .try_trimmed(parameter..=start)
        } else {
            self.try_extended_to(start..=parameter)?
                .try_trimmed(end..=parameter)
        }
    }

    fn validate_length_extension(&self, length: Real) -> Result<(), GeometryError> {
        if !length.is_finite() || length <= 0.0 {
            return Err(GeometryError::InvalidCurveExtensionLength);
        }
        if self.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen);
        }
        Ok(())
    }

    fn circular_extension_piece(
        &self,
        at_start: bool,
        length: Real,
        tolerance: Tolerance,
        match_source_degree: bool,
    ) -> Result<Self, GeometryError> {
        let domain = self.domain();
        let endpoint_parameter = if at_start {
            *domain.start()
        } else {
            *domain.end()
        };
        let endpoint = self.evaluate(endpoint_parameter)?;
        let derivative = self.derivative_at(endpoint_parameter)?;
        let speed = derivative.length()?;
        let tangent = derivative.normalized(tolerance)?;
        let parameter_delta = length / speed;
        require_finite([parameter_delta], "circular curve extension parameter")?;
        if parameter_delta == 0.0 {
            return Err(GeometryError::CurveExtensionLengthDidNotConverge);
        }
        let piece_domain = if at_start {
            (endpoint_parameter - parameter_delta)..=endpoint_parameter
        } else {
            endpoint_parameter..=(endpoint_parameter + parameter_delta)
        };
        require_finite(
            [*piece_domain.start(), *piece_domain.end()],
            "circular curve extension domain",
        )?;

        let curvature = CurveRef::NurbsCurve(self).curvature_vector(endpoint_parameter)?;
        let curvature_magnitude = curvature.length()?;
        let mut piece = if self.degree == 1 || curvature_magnitude == 0.0 {
            let degree = if match_source_degree { self.degree } else { 1 };
            let points = (0..=degree)
                .map(|index| {
                    let fraction = index as Real / degree as Real;
                    let offset = if at_start {
                        -length * (1.0 - fraction)
                    } else {
                        length * fraction
                    };
                    endpoint.translated(tangent.as_vector().scaled(offset)?)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut knots = vec![*piece_domain.start(); degree + 1];
            knots.extend(std::iter::repeat_n(*piece_domain.end(), degree + 1));
            Self::try_new(degree, points, knots)?
        } else {
            let arc = CircularArc3::try_from_start_tangent_curvature_length(
                endpoint,
                if at_start {
                    tangent.opposite()
                } else {
                    tangent
                },
                curvature,
                length,
                tolerance,
            )?;
            let arc = if at_start {
                arc.reversed(tolerance)?
            } else {
                arc
            };
            arc.to_nurbs()?.try_reparameterized(piece_domain)?
        };
        if match_source_degree && piece.degree < self.degree {
            piece = piece.try_change_degree(self.degree, false)?;
        }
        Ok(piece)
    }

    fn try_canonical_circular_arc(
        &self,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        if self.degree < 2 {
            return Ok(None);
        }
        let domain = self.domain();
        let start_parameter = *domain.start();
        let (start, derivative, _) = self.evaluate_with_second_derivative(start_parameter)?;
        let tangent = derivative.normalized(tolerance)?;
        let curvature = CurveRef::NurbsCurve(self).curvature_vector(start_parameter)?;
        let curvature_magnitude = curvature.length()?;
        if curvature_magnitude == 0.0 {
            return Ok(None);
        }
        let radius = 1.0 / curvature_magnitude;
        require_finite([radius], "canonical circular arc radius")?;
        let center =
            start.translated(curvature.normalized_nonzero()?.as_vector().scaled(radius)?)?;
        let radial = center.vector_to(start)?.normalized_nonzero()?;
        let normal = radial
            .as_vector()
            .cross(tangent.as_vector())?
            .normalized_nonzero()?;
        let center_scale = center
            .to_array()
            .into_iter()
            .chain([radius])
            .fold(1.0_f64, |scale, coordinate| scale.max(coordinate.abs()));
        let center_tolerance = tolerance
            .absolute()
            .max(tolerance.relative() * center_scale)
            * 8.0;

        for (span_start, span_end) in self.spans() {
            for parameter in [span_start, span_start * 0.5 + span_end * 0.5, span_end] {
                let point = self.evaluate(parameter)?;
                let sample_curvature = CurveRef::NurbsCurve(self).curvature_vector(parameter)?;
                let sample_magnitude = sample_curvature.length()?;
                if sample_magnitude == 0.0 {
                    return Ok(None);
                }
                let sample_center = point.translated(
                    sample_curvature.scaled(1.0 / (sample_magnitude * sample_magnitude))?,
                )?;
                if center.distance_to(sample_center)? > center_tolerance {
                    return Ok(None);
                }
                let sample_tangent = self.derivative_at(parameter)?.normalized(tolerance)?;
                let sample_radial = center.vector_to(point)?.normalized_nonzero()?;
                let orientation = sample_radial
                    .as_vector()
                    .cross(sample_tangent.as_vector())?
                    .dot(normal.as_vector())?;
                if orientation <= 0.0 {
                    return Ok(None);
                }
            }
        }

        let length = self.length(tolerance)?;
        let arc = match CircularArc3::try_from_start_tangent_curvature_length(
            start, tangent, curvature, length, tolerance,
        ) {
            Ok(arc) => arc,
            Err(GeometryError::Degenerate {
                context: "circular arc",
            }) => return Ok(None),
            Err(error) => return Err(error),
        };
        Ok(Some(arc.to_nurbs()?.try_reparameterized(domain)?))
    }

    /// Extracts the directed subcurve from `start` to `end`.
    ///
    /// Increasing parameters retain the natural curve direction. Decreasing
    /// parameters reverse an open subcurve; on a closed curve they select the
    /// forward portion that crosses the existing seam, matching Rhino's
    /// directed `SubCrv` selection behavior.
    pub fn try_subcurve(&self, start: Real, end: Real) -> Result<Self, GeometryError> {
        if !start.is_finite() || !end.is_finite() || start == end {
            return Err(GeometryError::InvalidCurveTrimInterval);
        }
        let domain = self.domain();
        let domain_start = *domain.start();
        let domain_end = *domain.end();
        if start < domain_start || start > domain_end || end < domain_start || end > domain_end {
            return Err(GeometryError::InvalidCurveTrimInterval);
        }
        if start < end {
            return self.try_trimmed(start..=end);
        }
        if !self.is_closed()? {
            return self.try_trimmed(end..=start)?.reversed();
        }
        if start == domain_end {
            return self.try_trimmed(domain_start..=end);
        }
        if end == domain_start {
            return self.try_trimmed(start..=domain_end);
        }

        self.try_closed_subcurve_across_seam(start, end)
    }

    fn try_closed_subcurve_across_seam(
        &self,
        start: Real,
        end: Real,
    ) -> Result<Self, GeometryError> {
        let domain = self.domain();
        let domain_start = *domain.start();
        let domain_end = *domain.end();
        let left = self.try_trimmed(start..=domain_end)?;
        let wrapped_end = translate_curve_parameter(end, domain_start, domain_end, 1)?;
        let right = self
            .try_trimmed(domain_start..=end)?
            .try_reparameterized(domain_end..=wrapped_end)?;
        left.try_append_clamped(&right)
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

    fn de_boor_side_controls_unbounded(
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
                let alpha = interval_fraction_unbounded(
                    parameter,
                    self.knots[knot_index],
                    self.knots[knot_index + self.degree - level + 1],
                )?;
                work[local_index] = blend_weighted_control_points_unbounded(
                    work[local_index - 1],
                    work[local_index],
                    alpha,
                )?;
            }
            left.push(work[level]);
            right.push(work[self.degree]);
        }
        right.reverse();
        Ok((left, right))
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
    // Preserve exactly representable affine maps before resorting to scaled
    // endpoints. Unconditional normalization can move an exact interior knot
    // by one ulp and create a zero-derivative sliver when trimming there.
    let source_width = source_end - source_start;
    let target_width = target_end - target_start;
    if source_width.is_finite() && source_width > 0.0 && target_width.is_finite() {
        let left = value - source_start;
        let right = source_end - value;
        let direct = if left.abs() <= right.abs() {
            crate::parameter::scaled_ratio(left, target_width, source_width)
                .map(|offset| target_start + offset)
        } else {
            crate::parameter::scaled_ratio(right, target_width, source_width)
                .map(|offset| target_end - offset)
        };
        if let Ok(mapped) = direct
            && mapped.is_finite()
        {
            return Ok(mapped);
        }
    }
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
    let alpha = interval_fraction_unbounded(value, interval_start, interval_end)?;
    // The validated span brackets `value`; a value just outside this range can
    // only be floating-point roundoff in the ratio calculation.
    Ok(alpha.clamp(0.0, 1.0))
}

fn interval_fraction_unbounded(
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
    Ok(alpha)
}

fn finite_midpoint(left: Real, right: Real) -> Real {
    if left.is_sign_negative() == right.is_sign_negative() {
        left + (right - left) * 0.5
    } else {
        left * 0.5 + right * 0.5
    }
}

fn farthest_coordinate(origin: Real, minimum: Real, maximum: Real) -> Real {
    if (origin - minimum).abs() >= (maximum - origin).abs() {
        minimum
    } else {
        maximum
    }
}

fn interpolate_parameter(start: Real, end: Real, fraction: Real) -> Real {
    if start.is_sign_negative() == end.is_sign_negative() {
        start + (end - start) * fraction
    } else {
        start * (1.0 - fraction) + end * fraction
    }
}

fn curve_span_overlap(
    first: &NurbsCurve,
    second: &NurbsCurve,
    distance_tolerance: Real,
    tolerance: Tolerance,
) -> Result<Option<CurveCurveOverlap>, GeometryError> {
    let mut boundaries = Vec::with_capacity(4);
    let first_domain = first.domain();
    for first_parameter in [*first_domain.start(), *first_domain.end()] {
        let first_point = first.evaluate(first_parameter)?;
        let second_parameter = second.closest_parameter(first_point, tolerance)?;
        let second_point = second.evaluate(second_parameter)?;
        if first_point.distance_to(second_point)? <= distance_tolerance {
            push_unique_curve_intersection(
                &mut boundaries,
                CurveCurveIntersection {
                    first_parameter,
                    second_parameter,
                    point: midpoint_between_points(first_point, second_point)?,
                },
                distance_tolerance,
            );
        }
    }
    let second_domain = second.domain();
    for second_parameter in [*second_domain.start(), *second_domain.end()] {
        let second_point = second.evaluate(second_parameter)?;
        let first_parameter = first.closest_parameter(second_point, tolerance)?;
        let first_point = first.evaluate(first_parameter)?;
        if first_point.distance_to(second_point)? <= distance_tolerance {
            push_unique_curve_intersection(
                &mut boundaries,
                CurveCurveIntersection {
                    first_parameter,
                    second_parameter,
                    point: midpoint_between_points(first_point, second_point)?,
                },
                distance_tolerance,
            );
        }
    }
    boundaries.sort_by(compare_curve_intersections);

    let mut best = None;
    for start_index in 0..boundaries.len() {
        for &end in &boundaries[start_index + 1..] {
            let start = boundaries[start_index];
            if intersection_parameter_near(start.first_parameter, end.first_parameter)
                || intersection_parameter_near(start.second_parameter, end.second_parameter)
            {
                continue;
            }
            let first_piece = first.try_trimmed(start.first_parameter..=end.first_parameter)?;
            let second_start = start.second_parameter.min(end.second_parameter);
            let second_end = start.second_parameter.max(end.second_parameter);
            let second_piece = second.try_trimmed(second_start..=second_end)?;
            if !curve_span_lies_on_curve(
                &first_piece,
                &second_piece,
                distance_tolerance,
                tolerance,
            )? || !curve_span_lies_on_curve(
                &second_piece,
                &first_piece,
                distance_tolerance,
                tolerance,
            )? {
                continue;
            }
            let candidate = CurveCurveOverlap { start, end };
            if best.is_none_or(|existing: CurveCurveOverlap| {
                end.first_parameter - start.first_parameter
                    > existing.end.first_parameter - existing.start.first_parameter
            }) {
                best = Some(candidate);
            }
        }
    }
    Ok(best)
}

fn curve_span_lies_on_curve(
    candidate: &NurbsCurve,
    container: &NurbsCurve,
    distance_tolerance: Real,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    const MAX_OVERLAP_CERTIFICATE_SAMPLES: usize = 4096;
    let Some(intersection_bound) = candidate.degree().checked_mul(container.degree()) else {
        return Ok(false);
    };
    // Two rational Bezier loci without a shared component have at most the
    // product of their degrees in common. One more on-curve sample therefore
    // distinguishes an overlap from a finite set of crossings.
    let sample_intervals = intersection_bound.max(5);
    if sample_intervals >= MAX_OVERLAP_CERTIFICATE_SAMPLES {
        return Ok(false);
    }
    let domain = candidate.domain();
    for sample in 0..=sample_intervals {
        let fraction = sample as Real / sample_intervals as Real;
        let parameter = interpolate_parameter(*domain.start(), *domain.end(), fraction);
        let point = candidate.evaluate(parameter)?;
        let container_parameter = container.closest_parameter(point, tolerance)?;
        if point.distance_to(container.evaluate(container_parameter)?)? > distance_tolerance {
            return Ok(false);
        }
    }
    Ok(true)
}

fn midpoint_between_points(left: Point3, right: Point3) -> Result<Point3, GeometryError> {
    Point3::try_new(
        finite_midpoint(left.x(), right.x()),
        finite_midpoint(left.y(), right.y()),
        finite_midpoint(left.z(), right.z()),
    )
}

fn curve_pair_distance_tolerance(
    first: &NurbsCurve,
    second: &NurbsCurve,
    tolerance: Tolerance,
) -> Real {
    let coordinate_scale = first
        .control_points
        .iter()
        .chain(&second.control_points)
        .flat_map(|control| control.point.to_array())
        .fold(1.0_f64, |scale, coordinate| scale.max(coordinate.abs()));
    tolerance
        .absolute()
        .max(tolerance.relative() * coordinate_scale)
}

fn compare_curve_intersections(
    left: &CurveCurveIntersection,
    right: &CurveCurveIntersection,
) -> std::cmp::Ordering {
    left.first_parameter
        .total_cmp(&right.first_parameter)
        .then_with(|| left.second_parameter.total_cmp(&right.second_parameter))
}

fn compare_curve_overlaps(
    left: &CurveCurveOverlap,
    right: &CurveCurveOverlap,
) -> std::cmp::Ordering {
    compare_curve_intersections(&left.start, &right.start)
        .then_with(|| compare_curve_intersections(&left.end, &right.end))
}

fn curve_intersection_event_parameter(event: CurveCurveIntersectionEvent) -> Real {
    match event {
        CurveCurveIntersectionEvent::Point(intersection) => intersection.first_parameter,
        CurveCurveIntersectionEvent::Overlap(overlap) => overlap.start.first_parameter,
    }
}

fn push_unique_curve_overlap(
    overlaps: &mut Vec<CurveCurveOverlap>,
    overlap: CurveCurveOverlap,
    distance_tolerance: Real,
) {
    let duplicate = overlaps.iter().any(|existing| {
        curve_intersections_match(existing.start, overlap.start, distance_tolerance)
            && curve_intersections_match(existing.end, overlap.end, distance_tolerance)
    });
    if !duplicate {
        overlaps.push(overlap);
    }
}

fn merge_adjacent_curve_overlaps(
    overlaps: Vec<CurveCurveOverlap>,
    distance_tolerance: Real,
) -> Vec<CurveCurveOverlap> {
    let mut merged: Vec<CurveCurveOverlap> = Vec::with_capacity(overlaps.len());
    for overlap in overlaps {
        if let Some(previous) = merged.last_mut()
            && curve_intersections_match(previous.end, overlap.start, distance_tolerance)
        {
            previous.end = overlap.end;
            continue;
        }
        merged.push(overlap);
    }
    merged
}

fn curve_overlap_contains_intersection(
    overlap: CurveCurveOverlap,
    intersection: CurveCurveIntersection,
) -> bool {
    parameter_inside_intersection_interval(
        intersection.first_parameter,
        overlap.start.first_parameter,
        overlap.end.first_parameter,
    ) && parameter_inside_intersection_interval(
        intersection.second_parameter,
        overlap.start.second_parameter,
        overlap.end.second_parameter,
    )
}

fn parameter_inside_intersection_interval(value: Real, start: Real, end: Real) -> bool {
    let minimum = start.min(end);
    let maximum = start.max(end);
    (value >= minimum || intersection_parameter_near(value, minimum))
        && (value <= maximum || intersection_parameter_near(value, maximum))
}

fn curve_intersections_match(
    left: CurveCurveIntersection,
    right: CurveCurveIntersection,
    distance_tolerance: Real,
) -> bool {
    left.point
        .distance_to(right.point)
        .is_ok_and(|distance| distance <= distance_tolerance * 2.0)
        && intersection_parameter_near(left.first_parameter, right.first_parameter)
        && intersection_parameter_near(left.second_parameter, right.second_parameter)
}

fn push_unique_curve_intersection(
    intersections: &mut Vec<CurveCurveIntersection>,
    intersection: CurveCurveIntersection,
    distance_tolerance: Real,
) {
    if !intersections
        .iter()
        .any(|existing| curve_intersections_match(*existing, intersection, distance_tolerance))
    {
        intersections.push(intersection);
    }
}

fn parameter_near(left: Real, right: Real) -> bool {
    let scale = left.abs().max(right.abs()).max(1.0);
    (left - right).abs() <= Real::EPSILON * scale * 256.0
}

fn intersection_parameter_near(left: Real, right: Real) -> bool {
    let scale = left.abs().max(right.abs()).max(1.0);
    (left - right).abs() <= Real::EPSILON.sqrt() * scale * 8.0
}

fn partition_curve_span_at(
    start: Real,
    end: Real,
    additions: impl IntoIterator<Item = Real>,
) -> Vec<Real> {
    let mut parameters = vec![start, end];
    for addition in additions {
        parameters.push(addition.clamp(start, end));
    }
    parameters.sort_by(Real::total_cmp);
    parameters.dedup_by(|left, right| parameter_near(*left, *right));
    parameters
}

fn bounding_boxes_overlap(first: BoundingBox3, second: BoundingBox3, padding: Real) -> bool {
    let first_min = first.min().to_array();
    let first_max = first.max().to_array();
    let second_min = second.min().to_array();
    let second_max = second.max().to_array();
    (0..3).all(|axis| {
        first_min[axis] <= second_max[axis] + padding
            && second_min[axis] <= first_max[axis] + padding
    })
}

/// Uses a small, bounded subdivision tree to prove that two positive-weight
/// rational spans cannot meet. This is deliberately only an early-out: any
/// branch whose refined control hulls still overlap is left to the complete
/// intersection and tangency machinery.
fn curve_nodes_are_certifiably_disjoint(
    first: &CurveIntersectionNode,
    second: &CurveIntersectionNode,
    padding: Real,
) -> Result<bool, GeometryError> {
    const MAX_ADDITIONAL_DEPTH: u8 = 4;

    if !first.convex_hull_bounds || !second.convex_hull_bounds {
        return Ok(false);
    }
    let maximum_combined_depth = first
        .depth
        .saturating_add(second.depth)
        .saturating_add(MAX_ADDITIONAL_DEPTH);
    let mut stack = vec![(first.clone(), second.clone())];
    while let Some((first, second)) = stack.pop() {
        if !bounding_boxes_overlap(first.bounds, second.bounds, padding)
            || !curve_control_hulls_overlap_on_local_axes(&first.curve, &second.curve, padding)?
        {
            continue;
        }
        if first.depth.saturating_add(second.depth) >= maximum_combined_depth {
            return Ok(false);
        }

        if first.spatial_size()? >= second.spatial_size()? {
            let [left, right] = first.split()?;
            stack.push((right, second.clone()));
            stack.push((left, second));
        } else {
            let [left, right] = second.split()?;
            stack.push((first.clone(), right));
            stack.push((first, left));
        }
    }
    Ok(true)
}

fn curve_control_hulls_overlap_on_local_axes(
    first: &NurbsCurve,
    second: &NurbsCurve,
    padding: Real,
) -> Result<bool, GeometryError> {
    let first_domain = first.domain();
    let second_domain = second.domain();
    let first_parameter = finite_midpoint(*first_domain.start(), *first_domain.end());
    let second_parameter = finite_midpoint(*second_domain.start(), *second_domain.end());
    let (first_point, first_tangent, first_second) =
        first.evaluate_with_second_derivative(first_parameter)?;
    let (second_point, second_tangent, second_second) =
        second.evaluate_with_second_derivative(second_parameter)?;
    let residual = first_point.vector_to(second_point)?;
    let mut axes = vec![
        residual,
        first_tangent,
        second_tangent,
        first_tangent.cross(second_tangent)?,
    ];
    for (tangent, second_derivative) in [
        (first_tangent, first_second),
        (second_tangent, second_second),
    ] {
        let Ok(unit_tangent) = tangent.normalized_nonzero() else {
            continue;
        };
        let unit_tangent = unit_tangent.as_vector();
        for vector in [second_derivative, residual] {
            let along = vector.dot(unit_tangent)?;
            let tangent_part = unit_tangent.scaled(along)?;
            axes.push(Vector3::try_new(
                vector.x() - tangent_part.x(),
                vector.y() - tangent_part.y(),
                vector.z() - tangent_part.z(),
            )?);
        }
    }

    let origin = first.control_points()[0].point();
    for axis in axes {
        let Ok(axis) = axis.normalized_nonzero() else {
            continue;
        };
        let first_projection = curve_control_projection_bounds(first, origin, axis.as_vector())?;
        let second_projection = curve_control_projection_bounds(second, origin, axis.as_vector())?;
        if first_projection[0] > second_projection[1] + padding
            || second_projection[0] > first_projection[1] + padding
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn curve_control_projection_bounds(
    curve: &NurbsCurve,
    origin: Point3,
    axis: Vector3,
) -> Result<[Real; 2], GeometryError> {
    let mut minimum = Real::INFINITY;
    let mut maximum = Real::NEG_INFINITY;
    for control in curve.control_points() {
        let projection = origin.vector_to(control.point())?.dot(axis)?;
        minimum = minimum.min(projection);
        maximum = maximum.max(projection);
    }
    Ok([minimum, maximum])
}

fn closest_segment_fractions(
    first_start: Point3,
    first_end: Point3,
    second_start: Point3,
    second_end: Point3,
) -> Result<(Real, Real), GeometryError> {
    let first_direction = first_start.vector_to(first_end)?;
    let second_direction = second_start.vector_to(second_end)?;
    let offset = second_start.vector_to(first_start)?;
    let first_squared = first_direction.dot(first_direction)?;
    let second_squared = second_direction.dot(second_direction)?;
    let first_offset = first_direction.dot(offset)?;
    let second_offset = second_direction.dot(offset)?;
    let cross = first_direction.dot(second_direction)?;
    let tiny = Real::MIN_POSITIVE;

    if first_squared <= tiny && second_squared <= tiny {
        return Ok((0.0, 0.0));
    }
    if first_squared <= tiny {
        return Ok((0.0, (second_offset / second_squared).clamp(0.0, 1.0)));
    }
    if second_squared <= tiny {
        return Ok(((-first_offset / first_squared).clamp(0.0, 1.0), 0.0));
    }

    let determinant = first_squared * second_squared - cross * cross;
    let mut first = if determinant > Real::EPSILON * first_squared * second_squared * 32.0 {
        ((cross * second_offset - first_offset * second_squared) / determinant).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut second = (cross * first + second_offset) / second_squared;
    if second < 0.0 {
        second = 0.0;
        first = (-first_offset / first_squared).clamp(0.0, 1.0);
    } else if second > 1.0 {
        second = 1.0;
        first = ((cross - first_offset) / first_squared).clamp(0.0, 1.0);
    }
    Ok((first, second))
}

#[allow(clippy::too_many_arguments)]
fn refine_curve_curve_intersection(
    first: &NurbsCurve,
    second: &NurbsCurve,
    mut first_parameter: Real,
    mut second_parameter: Real,
    first_domain: [Real; 2],
    second_domain: [Real; 2],
    distance_tolerance: Real,
) -> Result<Option<CurveCurveIntersection>, GeometryError> {
    let mut distance = first
        .evaluate(first_parameter)?
        .distance_to(second.evaluate(second_parameter)?)?;
    for _ in 0..80 {
        let (first_point, first_derivative) = first.evaluate_with_derivative(first_parameter)?;
        let (second_point, second_derivative) =
            second.evaluate_with_derivative(second_parameter)?;
        let residual = second_point.vector_to(first_point)?;
        let first_squared = first_derivative.dot(first_derivative)?;
        let second_squared = second_derivative.dot(second_derivative)?;
        let cross = first_derivative.dot(second_derivative)?;
        let first_rhs = -first_derivative.dot(residual)?;
        let second_rhs = second_derivative.dot(residual)?;
        let determinant = first_squared * second_squared - cross * cross;
        let threshold = Real::EPSILON * first_squared * second_squared * 64.0;
        let (first_delta, second_delta) = if determinant.is_finite() && determinant > threshold {
            (
                (first_rhs * second_squared + cross * second_rhs) / determinant,
                (first_squared * second_rhs + cross * first_rhs) / determinant,
            )
        } else if first_squared >= second_squared && first_squared > 0.0 {
            (first_rhs / first_squared, 0.0)
        } else if second_squared > 0.0 {
            (0.0, second_rhs / second_squared)
        } else {
            break;
        };
        if !first_delta.is_finite() || !second_delta.is_finite() {
            break;
        }

        let mut factor: Real = 1.0;
        let mut accepted = None;
        for _ in 0..28 {
            let next_first = factor
                .mul_add(first_delta, first_parameter)
                .clamp(first_domain[0], first_domain[1]);
            let next_second = factor
                .mul_add(second_delta, second_parameter)
                .clamp(second_domain[0], second_domain[1]);
            if next_first == first_parameter && next_second == second_parameter {
                break;
            }
            let next_distance = first
                .evaluate(next_first)?
                .distance_to(second.evaluate(next_second)?)?;
            if next_distance <= distance {
                accepted = Some((next_first, next_second, next_distance));
                break;
            }
            factor *= 0.5;
        }
        let Some((next_first, next_second, next_distance)) = accepted else {
            break;
        };
        let parameter_converged = parameter_near(first_parameter, next_first)
            && parameter_near(second_parameter, next_second);
        first_parameter = next_first;
        second_parameter = next_second;
        distance = next_distance;
        if parameter_converged {
            break;
        }
    }
    let first_point = first.evaluate(first_parameter)?;
    let second_point = second.evaluate(second_parameter)?;
    if first_point.distance_to(second_point)? > distance_tolerance {
        return Ok(None);
    }
    (first_parameter, second_parameter) = snap_curve_intersection_to_domain_boundary(
        first,
        second,
        [first_parameter, second_parameter],
        first_domain,
        second_domain,
        distance_tolerance,
    )?;
    let first_point = first.evaluate(first_parameter)?;
    let second_point = second.evaluate(second_parameter)?;
    let point = Point3::try_new(
        finite_midpoint(first_point.x(), second_point.x()),
        finite_midpoint(first_point.y(), second_point.y()),
        finite_midpoint(first_point.z(), second_point.z()),
    )?;
    Ok(Some(CurveCurveIntersection {
        first_parameter,
        second_parameter,
        point,
    }))
}

fn snap_curve_intersection_to_domain_boundary(
    first: &NurbsCurve,
    second: &NurbsCurve,
    parameters: [Real; 2],
    first_domain: [Real; 2],
    second_domain: [Real; 2],
    distance_tolerance: Real,
) -> Result<(Real, Real), GeometryError> {
    let first_point = first.evaluate(parameters[0])?;
    let second_point = second.evaluate(parameters[1])?;
    let mut best = (
        0_usize,
        first_point.distance_to(second_point)?,
        parameters[0],
        parameters[1],
    );
    let mut consider =
        |snapped: usize, distance: Real, first_parameter: Real, second_parameter: Real| {
            if distance > distance_tolerance {
                return;
            }
            let parameters_precede = first_parameter
                .total_cmp(&best.2)
                .then_with(|| second_parameter.total_cmp(&best.3))
                .is_lt();
            let better = snapped > best.0
                || (snapped == best.0
                    && (distance < best.1 || (distance == best.1 && parameters_precede)));
            if better {
                best = (snapped, distance, first_parameter, second_parameter);
            }
        };

    for first_boundary in first_domain {
        if !intersection_parameter_near(parameters[0], first_boundary) {
            continue;
        }
        let first_point = first.evaluate(first_boundary)?;
        for second_boundary in second_domain {
            if intersection_parameter_near(parameters[1], second_boundary) {
                let distance = first_point.distance_to(second.evaluate(second_boundary)?)?;
                consider(2, distance, first_boundary, second_boundary);
            }
        }
    }

    let closest_tolerance = Tolerance::try_new(
        distance_tolerance,
        Real::EPSILON * 16.0,
        Real::EPSILON * 16.0,
    )?;
    for first_boundary in first_domain {
        if !intersection_parameter_near(parameters[0], first_boundary) {
            continue;
        }
        let first_point = first.evaluate(first_boundary)?;
        let (second_parameter, distance) = second.refine_closest_parameter(
            first_point,
            parameters[1],
            second_domain,
            closest_tolerance,
        )?;
        consider(1, distance, first_boundary, second_parameter);
    }
    for second_boundary in second_domain {
        if !intersection_parameter_near(parameters[1], second_boundary) {
            continue;
        }
        let second_point = second.evaluate(second_boundary)?;
        let (first_parameter, distance) = first.refine_closest_parameter(
            second_point,
            parameters[0],
            first_domain,
            closest_tolerance,
        )?;
        consider(1, distance, first_parameter, second_boundary);
    }
    Ok((best.2, best.3))
}

#[allow(clippy::too_many_arguments)]
fn initial_curve_curve_intersections(
    first: &NurbsCurve,
    second: &NurbsCurve,
    search_second: &NurbsCurve,
    first_domain: [Real; 2],
    second_domain: [Real; 2],
    refinement_tolerance: Real,
    distance_tolerance: Real,
    tolerance: Tolerance,
) -> Result<Vec<CurveCurveIntersection>, GeometryError> {
    const MIN_SAMPLE_INTERVALS: usize = 16;
    const MAX_SAMPLE_INTERVALS: usize = 256;
    let sample_intervals = first
        .degree()
        .checked_add(1)
        .and_then(|first_degree| {
            second
                .degree()
                .checked_add(1)
                .and_then(|second_degree| first_degree.checked_mul(second_degree))
        })
        .and_then(|product| product.checked_mul(4))
        .unwrap_or(MAX_SAMPLE_INTERVALS)
        .clamp(MIN_SAMPLE_INTERVALS, MAX_SAMPLE_INTERVALS);
    let closest_tolerance = Tolerance::try_new(
        refinement_tolerance,
        Real::EPSILON * 16.0,
        tolerance.angular(),
    )?;
    let mut samples = Vec::with_capacity(sample_intervals + 1);
    for index in 0..=sample_intervals {
        let fraction = index as Real / sample_intervals as Real;
        let parameter = interpolate_parameter(first_domain[0], first_domain[1], fraction);
        let point = first.evaluate(parameter)?;
        let second_parameter = search_second.closest_parameter(point, closest_tolerance)?;
        let distance = point.distance_to(second.evaluate(second_parameter)?)?;
        samples.push((parameter, distance));
    }

    let mut intersections = Vec::new();
    if let Some(intersection) = refine_tangent_curve_curve_intersection(
        first,
        second,
        search_second,
        first_domain,
        second_domain,
        refinement_tolerance,
        tolerance,
    )? {
        push_unique_curve_intersection(&mut intersections, intersection, distance_tolerance);
    }
    for index in 1..sample_intervals {
        if samples[index].1 > samples[index - 1].1 || samples[index].1 > samples[index + 1].1 {
            continue;
        }
        if let Some(intersection) = refine_tangent_curve_curve_intersection(
            first,
            second,
            search_second,
            [samples[index - 1].0, samples[index + 1].0],
            second_domain,
            refinement_tolerance,
            tolerance,
        )? {
            push_unique_curve_intersection(&mut intersections, intersection, distance_tolerance);
        }
    }
    Ok(intersections)
}

fn refine_tangent_curve_curve_intersection(
    first: &NurbsCurve,
    second: &NurbsCurve,
    search_second: &NurbsCurve,
    first_domain: [Real; 2],
    second_domain: [Real; 2],
    refinement_tolerance: Real,
    tolerance: Tolerance,
) -> Result<Option<CurveCurveIntersection>, GeometryError> {
    const GOLDEN_FRACTION: Real = 0.618_033_988_749_894_9;
    let closest_tolerance = Tolerance::try_new(
        refinement_tolerance,
        Real::EPSILON * 16.0,
        tolerance.angular(),
    )?;
    let closest_at = |first_parameter| {
        let first_point = first.evaluate(first_parameter)?;
        let second_parameter = search_second.closest_parameter(first_point, closest_tolerance)?;
        let second_point = second.evaluate(second_parameter)?;
        let distance = first_point.distance_to(second_point)?;
        let point = Point3::try_new(
            finite_midpoint(first_point.x(), second_point.x()),
            finite_midpoint(first_point.y(), second_point.y()),
            finite_midpoint(first_point.z(), second_point.z()),
        )?;
        Ok::<_, GeometryError>((
            CurveCurveIntersection {
                first_parameter,
                second_parameter,
                point,
            },
            distance,
        ))
    };

    let mut left = first_domain[0];
    let mut right = first_domain[1];
    let mut inner_left = right - GOLDEN_FRACTION * (right - left);
    let mut inner_right = left + GOLDEN_FRACTION * (right - left);
    let mut left_hit = closest_at(inner_left)?;
    let mut right_hit = closest_at(inner_right)?;
    let mut best = closest_at(left)?;
    for candidate in [closest_at(right)?, left_hit, right_hit] {
        if candidate.1 < best.1 {
            best = candidate;
        }
    }

    for _ in 0..80 {
        let parameter_scale = left.abs().max(right.abs()).max(1.0);
        if right - left <= Real::EPSILON * parameter_scale * 64.0 {
            break;
        }
        if left_hit.1 <= right_hit.1 {
            right = inner_right;
            inner_right = inner_left;
            right_hit = left_hit;
            inner_left = right - GOLDEN_FRACTION * (right - left);
            left_hit = closest_at(inner_left)?;
            if left_hit.1 < best.1 {
                best = left_hit;
            }
        } else {
            left = inner_left;
            inner_left = inner_right;
            left_hit = right_hit;
            inner_right = left + GOLDEN_FRACTION * (right - left);
            right_hit = closest_at(inner_right)?;
            if right_hit.1 < best.1 {
                best = right_hit;
            }
        }
    }

    let acceptance = refinement_tolerance * 4.0;
    if best.1 > acceptance {
        return Ok(None);
    }
    if let Some(refined) = refine_curve_curve_tangency(
        first,
        second,
        search_second,
        best.0,
        first_domain,
        closest_tolerance,
    )? && refined.1 <= acceptance
    {
        best = refined;
    }
    if let Some(refined) = refine_curve_curve_intersection(
        first,
        second,
        best.0.first_parameter,
        best.0.second_parameter,
        first_domain,
        second_domain,
        refinement_tolerance,
    )? {
        return Ok(Some(refined));
    }
    Ok(Some(best.0))
}

fn refine_curve_curve_tangency(
    first: &NurbsCurve,
    second: &NurbsCurve,
    search_second: &NurbsCurve,
    mut current: CurveCurveIntersection,
    first_domain: [Real; 2],
    tolerance: Tolerance,
) -> Result<Option<(CurveCurveIntersection, Real)>, GeometryError> {
    let (_, first_derivative) = first.evaluate_with_derivative(current.first_parameter)?;
    let (_, second_derivative) = second.evaluate_with_derivative(current.second_parameter)?;
    let cross = first_derivative.cross(second_derivative)?;
    let Ok(axis) = cross.normalized_nonzero() else {
        let distance = first
            .evaluate(current.first_parameter)?
            .distance_to(second.evaluate(current.second_parameter)?)?;
        return Ok(Some((current, distance)));
    };
    let axis = axis.as_vector();
    let (sample, mut current_value, mut current_distance) = curve_curve_tangency_sample(
        first,
        second,
        search_second,
        current.first_parameter,
        axis,
        tolerance,
    )?;
    current = sample;

    for _ in 0..16 {
        if current_value.abs() <= Real::EPSILON * 512.0 {
            break;
        }
        let parameter_scale = current
            .first_parameter
            .abs()
            .max((first_domain[1] - first_domain[0]).abs())
            .max(1.0);
        let difference_step = Real::EPSILON.sqrt() * parameter_scale * 8.0;
        let lower = (current.first_parameter - difference_step).max(first_domain[0]);
        let upper = (current.first_parameter + difference_step).min(first_domain[1]);
        if lower == upper {
            break;
        }
        let (_, lower_value, _) =
            curve_curve_tangency_sample(first, second, search_second, lower, axis, tolerance)?;
        let (_, upper_value, _) =
            curve_curve_tangency_sample(first, second, search_second, upper, axis, tolerance)?;
        let derivative = (upper_value - lower_value) / (upper - lower);
        if !derivative.is_finite() || derivative == 0.0 {
            break;
        }
        let delta = (-current_value / derivative).clamp(
            -(first_domain[1] - first_domain[0]) * 0.25,
            (first_domain[1] - first_domain[0]) * 0.25,
        );
        if !delta.is_finite() || delta == 0.0 {
            break;
        }

        let mut factor: Real = 1.0;
        let mut accepted = None;
        for _ in 0..20 {
            let parameter = factor
                .mul_add(delta, current.first_parameter)
                .clamp(first_domain[0], first_domain[1]);
            if parameter == current.first_parameter {
                break;
            }
            let candidate = curve_curve_tangency_sample(
                first,
                second,
                search_second,
                parameter,
                axis,
                tolerance,
            )?;
            if candidate.1.abs() < current_value.abs() {
                accepted = Some(candidate);
                break;
            }
            factor *= 0.5;
        }
        let Some((next, next_value, next_distance)) = accepted else {
            break;
        };
        current = next;
        current_value = next_value;
        current_distance = next_distance;
    }
    Ok(Some((current, current_distance)))
}

fn curve_curve_tangency_sample(
    first: &NurbsCurve,
    second: &NurbsCurve,
    search_second: &NurbsCurve,
    first_parameter: Real,
    axis: Vector3,
    tolerance: Tolerance,
) -> Result<(CurveCurveIntersection, Real, Real), GeometryError> {
    let (first_point, first_derivative) = first.evaluate_with_derivative(first_parameter)?;
    let second_parameter = search_second.closest_parameter(first_point, tolerance)?;
    let (second_point, second_derivative) = second.evaluate_with_derivative(second_parameter)?;
    let first_tangent = first_derivative.normalized_nonzero()?;
    let second_tangent = second_derivative.normalized_nonzero()?;
    let tangency = first_tangent
        .as_vector()
        .cross(second_tangent.as_vector())?
        .dot(axis)?;
    let distance = first_point.distance_to(second_point)?;
    let point = Point3::try_new(
        finite_midpoint(first_point.x(), second_point.x()),
        finite_midpoint(first_point.y(), second_point.y()),
        finite_midpoint(first_point.z(), second_point.z()),
    )?;
    Ok((
        CurveCurveIntersection {
            first_parameter,
            second_parameter,
            point,
        },
        tangency,
        distance,
    ))
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

fn change_bezier_end_weights(
    controls: &mut [WeightedPoint3],
    desired_start: Real,
    desired_end: Real,
) -> Result<(), GeometryError> {
    debug_assert!(controls.len() >= 2);
    let last = controls.len() - 1;
    let start_weight = controls[0].weight();
    let end_weight = controls[last].weight();
    if start_weight == desired_start && end_weight == desired_end {
        return Ok(());
    }
    let scale = desired_start / start_weight;
    let power = (desired_end / end_weight) / scale;
    if !scale.is_finite() || scale == 0.0 || !power.is_finite() || power <= 0.0 {
        return Err(GeometryError::InvalidControlNet {
            context: "Bezier end weights cannot be changed projectively",
        });
    }
    let ratio = power.powf(1.0 / last as Real);
    if !ratio.is_finite() || ratio <= 0.0 {
        return Err(GeometryError::NonFinite {
            context: "Bezier end-weight reparameterization ratio",
        });
    }
    for (index, control) in controls.iter_mut().enumerate() {
        let weight = control.weight() * scale * ratio.powf(index as Real);
        *control = WeightedPoint3::try_new(control.point(), weight)?;
    }
    controls[0] = WeightedPoint3::try_new(controls[0].point(), desired_start)?;
    controls[last] = WeightedPoint3::try_new(controls[last].point(), desired_end)?;
    Ok(())
}

fn blend_weighted_control_points(
    left: WeightedPoint3,
    right: WeightedPoint3,
    alpha: Real,
) -> Result<WeightedPoint3, GeometryError> {
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
        return Err(GeometryError::InvalidKnotVector {
            context: "knot-insertion blend factor is outside zero to one",
        });
    }

    blend_weighted_control_points_unbounded(left, right, alpha)
}

fn blend_weighted_control_points_unbounded(
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
    if !alpha.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "NURBS projective blend factor",
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
    fn closed_curve_seam_relocation_rejects_open_curves_and_wraps_finite_parameters() {
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
        let t = *closed.domain().end() + 1.0;
        let shifted = closed.try_change_closed_seam(t).unwrap();
        assert_eq!(*shifted.domain().start(), t);
        assert!(
            shifted
                .evaluate(t)
                .unwrap()
                .distance_to(closed.evaluate(*closed.domain().start() + 1.0).unwrap())
                .unwrap()
                < 1e-12
        );
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
    fn nurbs_end_weight_normalization_preserves_the_projective_locus() {
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(0.0, 0.0), 2.0).unwrap(),
                WeightedPoint3::try_new(point(3.0, 5.0), 3.0).unwrap(),
                WeightedPoint3::try_new(point(8.0, 1.0), 8.0).unwrap(),
            ],
            vec![2.0, 2.0, 2.0, 6.0, 6.0, 6.0],
        )
        .unwrap();
        let normalized = curve.try_normalized_end_weights().unwrap();
        assert_eq!(normalized.domain(), curve.domain());
        assert_eq!(normalized.knots(), curve.knots());
        assert_eq!(
            normalized
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            curve
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>()
        );
        for (control, expected) in normalized.control_points().iter().zip([1.0, 0.75, 1.0]) {
            assert!(Tolerance::DEFAULT.approx_eq(control.weight(), expected));
        }

        let projective_scale = 0.5;
        for sample in 0..=32 {
            let normalized_parameter = sample as Real / 32.0;
            let source_fraction = projective_scale * normalized_parameter
                / (1.0 - normalized_parameter + projective_scale * normalized_parameter);
            let source_parameter = 2.0 + 4.0 * source_fraction;
            let target_parameter = 2.0 + 4.0 * normalized_parameter;
            assert_point_near(
                curve.evaluate(source_parameter).unwrap(),
                normalized.evaluate(target_parameter).unwrap(),
            );
        }

        let multispan = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(0.0, 0.0), 2.0).unwrap(),
                WeightedPoint3::try_new(point(2.0, 3.0), 3.0).unwrap(),
                WeightedPoint3::try_new(point(4.0, 1.0), 5.0).unwrap(),
                WeightedPoint3::try_new(point(6.0, 0.0), 8.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
        )
        .unwrap();
        let normalized_multispan = multispan.try_normalized_end_weights().unwrap();
        assert_eq!(normalized_multispan.control_points()[0].weight(), 1.0);
        assert_eq!(normalized_multispan.control_points()[3].weight(), 1.0);
        assert_ne!(normalized_multispan.knots(), multispan.knots());
        for sample in 0..=32 {
            let normalized_parameter = sample as Real / 32.0;
            let source_fraction = 0.5 * normalized_parameter
                / (1.0 - normalized_parameter + 0.5 * normalized_parameter);
            assert_point_near(
                multispan
                    .evaluate(multispan.parameter_at(source_fraction).unwrap())
                    .unwrap(),
                normalized_multispan
                    .evaluate(
                        normalized_multispan
                            .parameter_at(normalized_parameter)
                            .unwrap(),
                    )
                    .unwrap(),
            );
        }
    }

    #[test]
    fn normalized_multispan_trim_retains_piecewise_bezier_breaks() {
        let curve = NurbsCurve::try_new_rational(
            2,
            [
                ([0.0, 2.0], 1.0),
                ([2.0, 3.0], 0.8),
                ([5.0, 6.0], 1.2),
                ([8.0, 7.5], 0.9),
                ([10.0, 8.0], 1.0),
            ]
            .into_iter()
            .map(|(coordinates, weight)| {
                WeightedPoint3::try_new(point(coordinates[0], coordinates[1]), weight).unwrap()
            })
            .collect(),
            vec![2.0, 2.0, 2.0, 3.0, 5.0, 6.0, 6.0, 6.0],
        )
        .unwrap();
        let end = 3.361_818_303_014_144;
        let trimmed = curve
            .try_trimmed_with_normalized_end_weights(2.0..=end)
            .unwrap();

        assert_eq!(trimmed.degree(), 2);
        assert_eq!(trimmed.knots(), &[2.0, 2.0, 2.0, 3.0, 3.0, end, end, end]);
        for ((control, expected_point), expected_weight) in trimmed
            .control_points()
            .iter()
            .zip([
                point(0.0, 2.0),
                point(2.0, 3.0),
                point(3.285_714_285_714_285_6, 4.285_714_285_714_286),
                point(3.664_855_640_638_289, 4.664_855_640_638_289),
                point(4.0, 4.970_966_978_942_94),
            ])
            .zip([
                1.0,
                0.8,
                0.933_333_333_333_333_2,
                0.974_514_160_436_564_2,
                1.0,
            ])
        {
            assert_point_near(control.point(), expected_point);
            assert!(Tolerance::DEFAULT.approx_eq(control.weight(), expected_weight));
        }
        for sample in 0..=32 {
            let point = trimmed
                .evaluate(trimmed.parameter_at(sample as Real / 32.0).unwrap())
                .unwrap();
            let source_parameter = curve.closest_parameter(point, Tolerance::DEFAULT).unwrap();
            assert!(source_parameter >= 2.0 && source_parameter <= end);
            assert_point_near(point, curve.evaluate(source_parameter).unwrap());
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
    fn full_multiplicity_kink_predicate_ignores_collinear_knots() {
        let curve = |middle_y| {
            NurbsCurve::try_new(
                1,
                vec![
                    Point3::try_new(0.0, 2.0, 0.0).unwrap(),
                    Point3::try_new(5.0, middle_y, 0.0).unwrap(),
                    Point3::try_new(10.0, 6.0, 0.0).unwrap(),
                ],
                vec![0.0, 0.0, 1.0, 2.0, 2.0],
            )
            .unwrap()
        };

        assert!(
            !curve(4.0)
                .has_full_multiplicity_kink(Tolerance::DEFAULT)
                .unwrap()
        );
        assert!(
            curve(7.0)
                .has_full_multiplicity_kink(Tolerance::DEFAULT)
                .unwrap()
        );
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
    fn natural_extension_extrapolates_the_endpoint_nurbs_spans_exactly() {
        let curve = NurbsCurve::try_new(
            2,
            vec![point(0.0, 0.0), point(1.0, 2.0), point(3.0, 1.0)],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let extended = curve.try_extended_to(-1.0..=2.0).unwrap();

        assert_eq!(extended.domain(), -1.0..=2.0);
        assert_eq!(extended.knots(), &[-1.0, -1.0, -1.0, 2.0, 2.0, 2.0]);
        for parameter in [-1.0, -0.5, 0.0, 0.3, 1.0, 1.5, 2.0] {
            assert_point_near(
                extended.evaluate(parameter).unwrap(),
                point(
                    parameter.mul_add(parameter, 2.0 * parameter),
                    (-3.0 * parameter).mul_add(parameter, 4.0 * parameter),
                ),
            );
        }
    }

    #[test]
    fn natural_extension_clamps_nonclamped_rational_ends_and_preserves_the_source() {
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
        let extended = curve.try_extended_to(-0.25..=2.25).unwrap();

        assert_eq!(extended.domain(), -0.25..=2.25);
        assert!(extended.knots()[..3].iter().all(|knot| *knot == -0.25));
        assert!(
            extended.knots()[extended.knots().len() - 3..]
                .iter()
                .all(|knot| *knot == 2.25)
        );
        for sample in 0..=32 {
            let parameter = 2.0 * sample as Real / 32.0;
            assert_point_near(
                extended.evaluate(parameter).unwrap(),
                curve.evaluate(parameter).unwrap(),
            );
        }
    }

    #[test]
    fn natural_extension_by_length_matches_rhino_smooth_extension_parameters() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 2.0, 1.0).unwrap(),
                Point3::try_new(3.0, 1.0, -1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();

        let start = curve
            .try_extended_by_length(CurveExtensionSide::Start, 1.75, Tolerance::DEFAULT)
            .unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(*start.domain().start(), -0.294_749_475_954_422_64));
        assert_eq!(*start.domain().end(), 1.0);
        assert!(
            Tolerance::DEFAULT.approx_eq(
                start
                    .try_trimmed(*start.domain().start()..=0.0)
                    .unwrap()
                    .length(Tolerance::DEFAULT)
                    .unwrap(),
                1.75
            )
        );

        let both = curve
            .try_extended_by_length(CurveExtensionSide::Both, 1.5, Tolerance::DEFAULT)
            .unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(*both.domain().start(), -0.258_483_515_102_362_95));
        assert!(Tolerance::DEFAULT.approx_eq(*both.domain().end(), 1.219_618_451_394_380_4));
    }

    #[test]
    fn rational_natural_extension_by_length_reaches_the_requested_distance() {
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 1.0), 0.6).unwrap(),
                WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let extended = curve
            .try_extended_by_length(CurveExtensionSide::End, 0.75, Tolerance::DEFAULT)
            .unwrap();
        assert!(
            (*extended.domain().end() - 2.087_387_394_770_060_7).abs() < 5.0e-9,
            "actual domain {:?}",
            extended.domain()
        );
        let extension = extended
            .try_trimmed(1.0..=*extended.domain().end())
            .unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(extension.length(Tolerance::DEFAULT).unwrap(), 0.75));
    }

    #[test]
    fn linear_extension_by_length_adds_degree_matched_tangent_spans() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 2.0, 1.0).unwrap(),
                Point3::try_new(3.0, 1.0, -1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let extended = curve
            .try_extended_linearly_by_length(CurveExtensionSide::Both, 1.5, Tolerance::DEFAULT)
            .unwrap();

        let start = -1.5 / 24.0_f64.sqrt();
        let end = 1.0 + 1.5 / 6.0;
        assert!(Tolerance::DEFAULT.approx_eq(*extended.domain().start(), start));
        assert_eq!(*extended.domain().end(), end);
        assert_eq!(extended.knots().len(), 10);
        assert!(
            extended.knots()[..3]
                .iter()
                .all(|knot| Tolerance::DEFAULT.approx_eq(*knot, start))
        );
        assert_eq!(&extended.knots()[3..], &[0.0, 0.0, 1.0, 1.0, end, end, end]);
        let expected = [
            [
                -0.612_372_435_695_794_6,
                -1.224_744_871_391_589_2,
                -0.612_372_435_695_794_6,
            ],
            [
                -0.306_186_217_847_897_3,
                -0.612_372_435_695_794_6,
                -0.306_186_217_847_897_3,
            ],
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 1.0],
            [3.0, 1.0, -1.0],
            [3.5, 0.75, -1.5],
            [4.0, 0.5, -2.0],
        ];
        assert_eq!(extended.control_points().len(), expected.len());
        for (control, expected) in extended.control_points().iter().zip(expected) {
            assert_point_near(control.point(), Point3::try_from(expected).unwrap());
            assert_eq!(control.weight(), 1.0);
        }
        assert!(
            Tolerance::DEFAULT.approx_eq(
                extended
                    .try_trimmed(*extended.domain().start()..=0.0)
                    .unwrap()
                    .length(Tolerance::DEFAULT)
                    .unwrap(),
                1.5,
            )
        );
        assert!(
            Tolerance::DEFAULT.approx_eq(
                extended
                    .try_trimmed(1.0..=*extended.domain().end())
                    .unwrap()
                    .length(Tolerance::DEFAULT)
                    .unwrap(),
                1.5,
            )
        );
    }

    #[test]
    fn linear_extension_clamps_rational_nonclamped_ends_without_moving_the_source() {
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(Point3::try_new(-2.0, 1.0, 0.0).unwrap(), 0.75).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(0.0, 4.0, 1.0).unwrap(), 2.0).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(5.0, -2.0, -1.0).unwrap(), 0.5).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(8.0, 3.0, 2.0).unwrap(), 1.25).unwrap(),
            ],
            vec![-1.0, -1.0, 0.0, 0.8, 2.0, 3.0, 3.0],
        )
        .unwrap();
        let extended = curve
            .try_extended_linearly_by_length(CurveExtensionSide::Both, 0.5, Tolerance::DEFAULT)
            .unwrap();

        assert_eq!(extended.control_points().len(), 8);
        assert!(
            extended.control_points()[..3]
                .iter()
                .all(|control| control.weight() == extended.control_points()[2].weight())
        );
        assert!(
            extended.control_points()[5..]
                .iter()
                .all(|control| control.weight() == extended.control_points()[5].weight())
        );
        for sample in 0..=32 {
            let parameter = 2.0 * sample as Real / 32.0;
            assert_point_near(
                extended.evaluate(parameter).unwrap(),
                curve.evaluate(parameter).unwrap(),
            );
        }
    }

    #[test]
    fn curve_intersection_refines_transverse_nurbs_spans() {
        let horizontal = NurbsCurve::try_new(
            1,
            vec![point(-2.0, 1.0), point(8.0, 1.0)],
            vec![0.0, 0.0, 10.0, 10.0],
        )
        .unwrap();
        let vertical = NurbsCurve::try_new(
            1,
            vec![point(3.0, -4.0), point(3.0, 6.0)],
            vec![-5.0, -5.0, 5.0, 5.0],
        )
        .unwrap();
        let intersections = horizontal
            .intersections_with_curve(&vertical, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(intersections.len(), 1);
        assert_point_near(intersections[0].point(), point(3.0, 1.0));
        assert!(Tolerance::DEFAULT.approx_eq(intersections[0].first_parameter(), 5.0));
        assert!(Tolerance::DEFAULT.approx_eq(intersections[0].second_parameter(), 0.0));
    }

    #[test]
    fn curve_intersection_finds_an_interior_tangent_contact() {
        let parabola = NurbsCurve::try_new(
            2,
            vec![point(0.0, 0.0), point(5.0, 8.0), point(10.0, 0.0)],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let tangent = NurbsCurve::try_new(
            1,
            vec![point(0.0, 4.0), point(10.0, 4.0)],
            vec![0.0, 0.0, 10.0, 10.0],
        )
        .unwrap();
        let intersections = parabola
            .intersections_with_curve(&tangent, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(intersections.len(), 1, "{intersections:#?}");
        assert_point_near(intersections[0].point(), point(5.0, 4.0));
        assert!(Tolerance::DEFAULT.approx_eq(intersections[0].first_parameter(), 0.5));
        assert!(Tolerance::DEFAULT.approx_eq(intersections[0].second_parameter(), 5.0));
    }

    #[test]
    fn curve_intersection_refines_an_off_center_rational_tangency() {
        let arc = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 1.0), 0.5_f64.sqrt()).unwrap(),
                WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let parameter = 0.3;
        let (contact, derivative) = arc.evaluate_with_derivative(parameter).unwrap();
        let tangent = derivative.normalized_nonzero().unwrap();
        let offset = tangent.as_vector().scaled(3.0).unwrap();
        let line = NurbsCurve::try_new(
            1,
            vec![
                contact.translated(offset.scaled(-1.0).unwrap()).unwrap(),
                contact.translated(offset).unwrap(),
            ],
            vec![-3.0, -3.0, 3.0, 3.0],
        )
        .unwrap();

        let intersections = arc
            .intersections_with_curve(&line, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(intersections.len(), 1, "{intersections:#?}");
        assert_point_near(intersections[0].point(), contact);
        assert!((intersections[0].first_parameter() - parameter).abs() < 1.0e-11);
        assert!(intersections[0].second_parameter().abs() < 1.0e-11);
    }

    #[test]
    fn curve_intersection_finds_a_rational_span_endpoint_tangent_to_a_polyline() {
        let weight = std::f64::consts::FRAC_1_SQRT_2;
        let circle = NurbsCurve::try_new_rational(
            2,
            [
                ([8.0, 5.0], 1.0),
                ([8.0, 6.5], weight),
                ([6.5, 6.5], 1.0),
                ([5.0, 6.5], weight),
                ([5.0, 5.0], 1.0),
                ([5.0, 3.5], weight),
                ([6.5, 3.5], 1.0),
                ([8.0, 3.5], weight),
                ([8.0, 5.0], 1.0),
            ]
            .into_iter()
            .map(|(coordinates, weight)| {
                WeightedPoint3::try_new(point(coordinates[0], coordinates[1]), weight).unwrap()
            })
            .collect(),
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0],
        )
        .unwrap();
        let polygon = NurbsCurve::try_new(
            1,
            vec![
                point(2.0, 3.0),
                point(5.0, 3.0),
                point(5.0, 7.0),
                point(2.0, 7.0),
                point(2.0, 3.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        )
        .unwrap();

        let circle_first = circle
            .intersections_with_curve(&polygon, Tolerance::DEFAULT)
            .unwrap();
        let polygon_first = polygon
            .intersections_with_curve(&circle, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(circle_first.len(), 1, "{circle_first:#?}");
        assert_eq!(polygon_first.len(), 1, "{polygon_first:#?}");
        assert!((circle_first[0].first_parameter() - 2.0).abs() < 1.0e-10);
        assert!((circle_first[0].second_parameter() - 1.5).abs() < 1.0e-10);
        assert!((polygon_first[0].first_parameter() - 1.5).abs() < 1.0e-10);
        assert!((polygon_first[0].second_parameter() - 2.0).abs() < 1.0e-10);
        assert_point_near(circle_first[0].point(), point(5.0, 5.0));
        assert_point_near(polygon_first[0].point(), point(5.0, 5.0));

        let separated = NurbsCurve::try_new(
            1,
            vec![
                point(2.0, 3.0),
                point(5.0 - 1.0e-8, 3.0),
                point(5.0 - 1.0e-8, 7.0),
                point(2.0, 7.0),
                point(2.0, 3.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        )
        .unwrap();
        assert!(
            circle
                .intersections_with_curve(&separated, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
        assert!(
            separated
                .intersections_with_curve(&circle, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn curve_intersection_certifies_nested_rational_arcs_are_disjoint() {
        let quarter_circle = |radius: Real| {
            NurbsCurve::try_new_rational(
                2,
                vec![
                    WeightedPoint3::try_new(point(5.0 + radius, 5.0), 1.0).unwrap(),
                    WeightedPoint3::try_new(point(5.0 + radius, 5.0 + radius), 0.5_f64.sqrt())
                        .unwrap(),
                    WeightedPoint3::try_new(point(5.0, 5.0 + radius), 1.0).unwrap(),
                ],
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            )
            .unwrap()
        };
        let outer = quarter_circle(3.0);
        let inner = quarter_circle(1.5);
        let outer_node = CurveIntersectionNode::try_new(outer.clone(), 0).unwrap();
        let inner_node = CurveIntersectionNode::try_new(inner.clone(), 0).unwrap();

        assert!(
            curve_nodes_are_certifiably_disjoint(
                &outer_node,
                &inner_node,
                Tolerance::DEFAULT.absolute(),
            )
            .unwrap()
        );
        assert!(
            outer
                .intersections_with_curve(&inner, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn curve_intersection_finds_two_tangencies_in_one_span_pair() {
        let double_contact = NurbsCurve::try_new(
            4,
            vec![
                point(0.0, 0.441),
                point(0.25, -0.609),
                point(0.5, 0.707_666_666_666_666_7),
                point(0.75, -0.609),
                point(1.0, 0.441),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let line = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(1.0, 0.0)],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();

        let intersections = double_contact
            .intersections_with_curve(&line, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(intersections.len(), 2, "{intersections:#?}");
        assert!((intersections[0].first_parameter() - 0.3).abs() < 1.0e-10);
        assert!((intersections[1].first_parameter() - 0.7).abs() < 1.0e-10);
        assert_point_near(intersections[0].point(), point(0.3, 0.0));
        assert_point_near(intersections[1].point(), point(0.7, 0.0));
    }

    #[test]
    fn curve_intersection_returns_collinear_overlap_endpoints() {
        let long = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(20.0, 0.0)],
            vec![0.0, 0.0, 20.0, 20.0],
        )
        .unwrap();
        let short = NurbsCurve::try_new(
            1,
            vec![point(10.0, 0.0), point(15.0, 0.0)],
            vec![-2.0, -2.0, 3.0, 3.0],
        )
        .unwrap();
        let intersections = long
            .intersections_with_curve(&short, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(intersections.len(), 2, "{intersections:#?}");
        assert_point_near(intersections[0].point(), point(10.0, 0.0));
        assert_point_near(intersections[1].point(), point(15.0, 0.0));
        assert!(Tolerance::DEFAULT.approx_eq(intersections[0].first_parameter(), 10.0));
        assert!(Tolerance::DEFAULT.approx_eq(intersections[1].first_parameter(), 15.0));
        assert!(Tolerance::DEFAULT.approx_eq(intersections[0].second_parameter(), -2.0));
        assert!(Tolerance::DEFAULT.approx_eq(intersections[1].second_parameter(), 3.0));
    }

    #[test]
    fn curve_intersection_events_preserve_partial_overlap_intervals() {
        let first = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(10.0, 0.0)],
            vec![0.0, 0.0, 10.0, 10.0],
        )
        .unwrap();
        let second = NurbsCurve::try_new(
            1,
            vec![point(5.0, 0.0), point(15.0, 0.0)],
            vec![20.0, 20.0, 30.0, 30.0],
        )
        .unwrap();

        let events = first
            .intersection_events_with_curve(&second, Tolerance::DEFAULT)
            .unwrap();
        let [CurveCurveIntersectionEvent::Overlap(overlap)] = events.as_slice() else {
            panic!("expected one overlap event, got {events:#?}")
        };
        assert_eq!(overlap.first_interval(), 5.0..=10.0);
        assert_eq!(overlap.second_interval(), 20.0..=25.0);
        assert_point_near(overlap.start().point(), point(5.0, 0.0));
        assert_point_near(overlap.end().point(), point(10.0, 0.0));
    }

    #[test]
    fn curve_intersection_events_merge_adjacent_overlap_spans() {
        let first = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(5.0, 0.0), point(10.0, 0.0)],
            vec![0.0, 0.0, 5.0, 10.0, 10.0],
        )
        .unwrap();
        let second = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(5.0, 0.0), point(10.0, 0.0)],
            vec![20.0, 20.0, 25.0, 30.0, 30.0],
        )
        .unwrap();

        let events = first
            .intersection_events_with_curve(&second, Tolerance::DEFAULT)
            .unwrap();
        let [CurveCurveIntersectionEvent::Overlap(overlap)] = events.as_slice() else {
            panic!("expected one merged overlap event, got {events:#?}")
        };
        assert_eq!(overlap.first_interval(), 0.0..=10.0);
        assert_eq!(overlap.second_interval(), 20.0..=30.0);
    }

    #[test]
    fn curve_intersection_events_keep_a_collinear_endpoint_as_a_point() {
        let first = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(5.0, 0.0)],
            vec![0.0, 0.0, 5.0, 5.0],
        )
        .unwrap();
        let second = NurbsCurve::try_new(
            1,
            vec![point(5.0, 0.0), point(10.0, 0.0)],
            vec![5.0, 5.0, 10.0, 10.0],
        )
        .unwrap();

        let events = first
            .intersection_events_with_curve(&second, Tolerance::DEFAULT)
            .unwrap();
        let [CurveCurveIntersectionEvent::Point(intersection)] = events.as_slice() else {
            panic!("expected one endpoint event, got {events:#?}")
        };
        assert_point_near(intersection.point(), point(5.0, 0.0));
    }

    #[test]
    fn boundary_extension_uses_the_nearest_hit_beyond_each_line_end() {
        let source = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(5.0, 0.0)],
            vec![0.0, 0.0, 5.0, 5.0],
        )
        .unwrap();
        let boundary = |x| {
            NurbsCurve::try_new(
                1,
                vec![point(x, -5.0), point(x, 5.0)],
                vec![0.0, 0.0, 10.0, 10.0],
            )
            .unwrap()
        };
        let extended = source
            .try_merged_to_curve_boundaries(
                CurveExtensionSide::Both,
                CurveExtensionStyle::Line,
                &[boundary(12.0), boundary(-3.0), boundary(8.0)],
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(extended.degree(), 1);
        assert!(Tolerance::DEFAULT.approx_eq(*extended.domain().start(), -3.0));
        assert!(Tolerance::DEFAULT.approx_eq(*extended.domain().end(), 8.0));
        assert_point_near(
            extended.evaluate(*extended.domain().start()).unwrap(),
            point(-3.0, 0.0),
        );
        assert_point_near(
            extended.evaluate(*extended.domain().end()).unwrap(),
            point(8.0, 0.0),
        );

        assert_eq!(
            source.try_merged_to_curve_boundaries(
                CurveExtensionSide::End,
                CurveExtensionStyle::Line,
                &[boundary(2.0)],
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::CurveExtensionBoundaryNotFound)
        );
    }

    #[test]
    fn boundary_extension_intersects_surfaces_and_trimmed_breps() {
        let source = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(5.0, 0.0)],
            vec![0.0, 0.0, 5.0, 5.0],
        )
        .unwrap();
        let surface = NurbsSurface::try_bilinear([
            Point3::try_new(10.0, -5.0, -5.0).unwrap(),
            Point3::try_new(10.0, 5.0, -5.0).unwrap(),
            Point3::try_new(10.0, 5.0, 5.0).unwrap(),
            Point3::try_new(10.0, -5.0, 5.0).unwrap(),
        ])
        .unwrap();
        let extended = source
            .try_merged_to_boundaries(
                CurveExtensionSide::End,
                CurveExtensionStyle::Line,
                &[CurveExtensionBoundary::Surface(surface)],
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_point_near(
            extended.evaluate(*extended.domain().end()).unwrap(),
            point(10.0, 0.0),
        );

        let frame = Frame3::try_from_normal(
            point(0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let box_boundary = Brep::try_box(
            frame,
            [[12.0, 14.0], [-2.0, 2.0], [-2.0, 2.0]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let extended = source
            .try_merged_to_boundaries(
                CurveExtensionSide::End,
                CurveExtensionStyle::Line,
                &[CurveExtensionBoundary::Brep(box_boundary)],
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_point_near(
            extended.evaluate(*extended.domain().end()).unwrap(),
            point(12.0, 0.0),
        );
    }

    #[test]
    fn boundary_extension_respects_curvature_and_brep_trim_holes() {
        let frame = Frame3::try_from_normal(
            point(0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame, 2.0, -2.0, 2.0).unwrap();
        let radial_source = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(0.5, 0.0)],
            vec![0.0, 0.0, 0.5, 0.5],
        )
        .unwrap();
        let extended = radial_source
            .try_merged_to_boundaries(
                CurveExtensionSide::End,
                CurveExtensionStyle::Line,
                &[CurveExtensionBoundary::Surface(cylinder)],
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_point_near(
            extended.evaluate(*extended.domain().end()).unwrap(),
            point(2.0, 0.0),
        );

        let outer = Polyline3::try_new(
            vec![
                Point3::try_new(10.0, -3.0, -3.0).unwrap(),
                Point3::try_new(10.0, 3.0, -3.0).unwrap(),
                Point3::try_new(10.0, 3.0, 3.0).unwrap(),
                Point3::try_new(10.0, -3.0, 3.0).unwrap(),
                Point3::try_new(10.0, -3.0, -3.0).unwrap(),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let hole = crate::Circle3::try_new(
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            1.0,
            crate::UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let holed_face =
            Brep::try_planar_face_with_holes(&outer, &[hole], Tolerance::DEFAULT).unwrap();
        let farther_surface = NurbsSurface::try_bilinear([
            Point3::try_new(12.0, -3.0, -3.0).unwrap(),
            Point3::try_new(12.0, 3.0, -3.0).unwrap(),
            Point3::try_new(12.0, 3.0, 3.0).unwrap(),
            Point3::try_new(12.0, -3.0, 3.0).unwrap(),
        ])
        .unwrap();
        let extended = radial_source
            .try_merged_to_boundaries(
                CurveExtensionSide::End,
                CurveExtensionStyle::Line,
                &[
                    CurveExtensionBoundary::Brep(holed_face),
                    CurveExtensionBoundary::Surface(farther_surface),
                ],
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_point_near(
            extended.evaluate(*extended.domain().end()).unwrap(),
            point(12.0, 0.0),
        );
    }

    #[test]
    fn boundary_extension_enters_a_coplanar_trim_before_the_underlying_surface_edge() {
        let source = NurbsCurve::try_new(
            1,
            vec![point(0.0, 1.0), point(5.0, 1.0)],
            vec![0.0, 0.0, 5.0, 5.0],
        )
        .unwrap();
        let diamond = Polyline3::try_new(
            vec![
                point(12.0, -3.0),
                point(15.0, 0.0),
                point(12.0, 3.0),
                point(9.0, 0.0),
                point(12.0, -3.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let face = Brep::try_planar_face_with_holes(&diamond, &[], Tolerance::DEFAULT).unwrap();
        let extended = source
            .try_merged_to_boundaries(
                CurveExtensionSide::End,
                CurveExtensionStyle::Line,
                &[CurveExtensionBoundary::Brep(face)],
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_point_near(
            extended.evaluate(*extended.domain().end()).unwrap(),
            point(10.0, 1.0),
        );
    }

    #[test]
    fn smooth_boundary_extension_matches_exact_nurbs_extrapolation() {
        let source = NurbsCurve::try_new(
            2,
            vec![point(0.0, 0.0), point(2.0, 3.0), point(4.0, 0.0)],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let boundary = NurbsCurve::try_new(
            1,
            vec![point(7.0, -20.0), point(7.0, 20.0)],
            vec![0.0, 0.0, 40.0, 40.0],
        )
        .unwrap();
        let extended = source
            .try_merged_to_curve_boundaries(
                CurveExtensionSide::End,
                CurveExtensionStyle::Smooth,
                &[boundary],
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert!((*extended.domain().end() - 1.75).abs() <= 2.0e-12);
        assert_point_near(extended.control_points()[2].point(), point(7.0, -7.875));
    }

    #[test]
    fn circular_boundary_extension_hits_the_exact_osculating_arc() {
        let source = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 1.0), std::f64::consts::FRAC_1_SQRT_2).unwrap(),
                WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let boundary = NurbsCurve::try_new(
            1,
            vec![point(-0.5, -2.0), point(-0.5, 2.0)],
            vec![0.0, 0.0, 4.0, 4.0],
        )
        .unwrap();
        let joined = source
            .try_joined_to_curve_boundaries(
                CurveExtensionSide::End,
                CurveExtensionStyle::Arc,
                std::slice::from_ref(&boundary),
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(joined.degree(), 2);
        assert_eq!(joined.knot_multiplicity(1.0).unwrap(), 2);
        assert_point_near(
            joined.evaluate(*joined.domain().end()).unwrap(),
            point(-0.5, 3.0_f64.sqrt() * 0.5),
        );
        assert!(
            (joined
                .try_trimmed(1.0..=*joined.domain().end())
                .unwrap()
                .length(Tolerance::DEFAULT)
                .unwrap()
                - std::f64::consts::FRAC_PI_6)
                .abs()
                < 1.0e-11
        );

        let merged = source
            .try_merged_to_curve_boundaries(
                CurveExtensionSide::End,
                CurveExtensionStyle::Natural,
                &[boundary],
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert!(
            merged
                .try_canonical_circular_arc(Tolerance::DEFAULT)
                .unwrap()
                .is_some()
        );
        assert_point_near(
            merged.evaluate(*merged.domain().end()).unwrap(),
            point(-0.5, 3.0_f64.sqrt() * 0.5),
        );
    }

    #[test]
    fn circular_extension_uses_the_exact_endpoint_osculating_arc() {
        let curve = NurbsCurve::try_new(
            2,
            vec![point(0.0, 0.0), point(1.0, 2.0), point(3.0, 1.0)],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let extended = curve
            .try_extended_circularly_by_length(CurveExtensionSide::End, 2.0, Tolerance::DEFAULT)
            .unwrap();

        let end = 1.0 + 1.0 / 5.0_f64.sqrt();
        assert_eq!(extended.degree(), 2);
        assert!(Tolerance::DEFAULT.approx_eq(*extended.domain().end(), end));
        assert_eq!(extended.knot_multiplicity(1.0).unwrap(), 2);
        assert!(Tolerance::DEFAULT.approx_eq(
            extended.control_points()[3].weight(),
            0.975_103_993_210_479_4,
        ));
        assert_point_near(
            extended.control_points()[4].point(),
            Point3::try_new(4.533_130_546_115_57, -0.258_287_297_307_559_06, 0.0).unwrap(),
        );
        assert!(
            Tolerance::DEFAULT.approx_eq(
                extended
                    .try_trimmed(1.0..=end)
                    .unwrap()
                    .length(Tolerance::DEFAULT)
                    .unwrap(),
                2.0,
            )
        );

        let capped = curve
            .try_extended_circularly_by_length(CurveExtensionSide::End, 30.0, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(capped.control_points().len(), 11);
        assert_point_near(
            capped.evaluate(*capped.domain().end()).unwrap(),
            point(3.0, 1.0),
        );
        let capped_length = capped
            .try_trimmed(1.0..=*capped.domain().end())
            .unwrap()
            .length(Tolerance::DEFAULT)
            .unwrap();
        let circumference = std::f64::consts::TAU * 2.0 * 5.0_f64.sqrt();
        assert!(
            (capped_length - circumference).abs() < 1.0e-8,
            "actual full-circle length {capped_length}"
        );
    }

    #[test]
    fn circular_extension_elevates_a_zero_curvature_line_fallback() {
        let curve = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 0.0),
                point(1.0, 1.0),
                point(2.0, 0.0),
                point(3.0, -1.0),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let extended = curve
            .try_extended_circularly_by_length(CurveExtensionSide::End, 2.0, Tolerance::DEFAULT)
            .unwrap();

        assert_eq!(extended.degree(), 3);
        assert_eq!(extended.control_points().len(), 7);
        assert_eq!(extended.knot_multiplicity(1.0).unwrap(), 3);
        assert_point_near(
            extended.control_points()[6].point(),
            Point3::try_new(4.414_213_562_373_095, -2.414_213_562_373_095, 0.0).unwrap(),
        );
        assert!(
            extended.control_points()[3..]
                .iter()
                .all(|control| control.weight() == 1.0)
        );

        let separate = curve
            .try_separate_circular_extensions_by_length(
                CurveExtensionSide::End,
                2.0,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(separate.len(), 1);
        assert_eq!(separate[0].degree(), 1);
        assert_eq!(separate[0].domain(), 0.0..=1.0);
        assert_point_near(
            separate[0].control_points()[1].point(),
            extended.control_points()[6].point(),
        );

        let spatial = NurbsCurve::try_new(
            3,
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 3.0, 1.0).unwrap(),
                Point3::try_new(4.0, -2.0, 2.0).unwrap(),
                Point3::try_new(7.0, 1.0, 0.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let separate = spatial
            .try_separate_circular_extensions_by_length(
                CurveExtensionSide::End,
                2.0,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(separate[0].degree(), 2);
        assert_eq!(separate[0].control_points().len(), 3);
    }

    #[test]
    fn merged_circular_extension_rebuilds_one_same_radius_arc() {
        let source = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 1.0), std::f64::consts::FRAC_1_SQRT_2).unwrap(),
                WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let joined = source
            .try_joined_circularly_by_length(CurveExtensionSide::End, 0.5, Tolerance::DEFAULT)
            .unwrap();
        let merged = source
            .try_merged_circularly_by_length(CurveExtensionSide::End, 0.5, Tolerance::DEFAULT)
            .unwrap();
        let natural = source
            .try_merged_naturally_by_length(CurveExtensionSide::End, 0.5, Tolerance::DEFAULT)
            .unwrap();
        let smooth = source
            .try_extended_by_length(CurveExtensionSide::End, 0.5, Tolerance::DEFAULT)
            .unwrap();

        assert_eq!(joined.knot_multiplicity(1.0).unwrap(), 2);
        assert_eq!(natural, merged);
        assert_ne!(smooth, merged);
        assert_eq!(merged.degree(), 2);
        assert_eq!(merged.control_points().len(), 5);
        assert_eq!(merged.domain(), joined.domain());
        assert!(!merged.knots().contains(&1.0));
        assert!(
            Tolerance::DEFAULT
                .approx_eq(merged.control_points()[1].weight(), 0.868_960_162_057_560_4,)
        );
        assert_point_near(
            merged.control_points()[4].point(),
            Point3::try_new(-0.479_425_538_604_203, 0.877_582_561_890_372_5, 0.0).unwrap(),
        );
    }

    #[test]
    fn merged_linear_extension_simplifies_lines_and_extends_polyline_end_spans() {
        let straight = NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let merged = straight
            .try_merged_linearly_by_length(CurveExtensionSide::End, 2.0, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(merged.degree(), 1);
        assert_eq!(merged.domain(), 0.0..=1.125);
        assert_eq!(
            merged
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(12.0, 0.0, 0.0).unwrap(),
            ]
        );

        let polyline = NurbsCurve::try_new(
            1,
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 1.0, 0.0).unwrap(),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 2.0],
        )
        .unwrap();
        let merged = polyline
            .try_merged_linearly_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(merged.degree(), 1);
        assert_eq!(merged.control_points().len(), 3);
        assert_eq!(merged.domain(), 0.0..=3.0);
        assert_eq!(
            merged.control_points()[2].point(),
            Point3::try_new(1.0, 2.0, 0.0).unwrap()
        );
    }

    #[test]
    fn joined_linear_extension_uses_unit_polyline_spans() {
        let polyline = NurbsCurve::try_new(
            1,
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 3.0, 0.0).unwrap(),
            ],
            vec![0.0, 0.0, 2.0, 5.0, 5.0],
        )
        .unwrap();
        let joined = polyline
            .try_joined_linearly_by_length(CurveExtensionSide::Both, 1.5, Tolerance::DEFAULT)
            .unwrap();

        assert_eq!(joined.degree(), 1);
        assert_eq!(joined.domain(), 0.0..=4.0);
        assert_eq!(joined.knots(), &[0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0]);
        assert_eq!(
            joined
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            vec![
                Point3::try_new(-1.5, 0.0, 0.0).unwrap(),
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                Point3::try_new(2.0, 3.0, 0.0).unwrap(),
                Point3::try_new(2.0, 4.5, 0.0).unwrap(),
            ]
        );
    }

    #[test]
    fn joined_natural_extension_retains_source_with_a_full_seam() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 2.0, 0.0).unwrap(),
                Point3::try_new(3.0, 1.0, 0.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let joined = curve
            .try_joined_naturally_by_length(CurveExtensionSide::End, 2.0, Tolerance::DEFAULT)
            .unwrap();

        assert_eq!(joined.degree(), 2);
        assert_eq!(joined.knot_multiplicity(1.0).unwrap(), 2);
        assert!(
            Tolerance::DEFAULT.approx_eq(
                joined
                    .try_trimmed(1.0..=*joined.domain().end())
                    .unwrap()
                    .length(Tolerance::DEFAULT)
                    .unwrap(),
                2.0,
            )
        );
        for sample in 0..=32 {
            let parameter = sample as Real / 32.0;
            assert_point_near(
                joined.evaluate(parameter).unwrap(),
                curve.evaluate(parameter).unwrap(),
            );
        }
    }

    #[test]
    fn separate_linear_extensions_are_unit_domain_tangent_lines() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 2.0, 0.0).unwrap(),
                Point3::try_new(3.0, 1.0, 0.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let extensions = curve
            .try_separate_linear_extensions_by_length(
                CurveExtensionSide::Both,
                2.0,
                Tolerance::DEFAULT,
            )
            .unwrap();

        assert_eq!(extensions.len(), 2);
        for extension in &extensions {
            assert_eq!(extension.degree(), 1);
            assert_eq!(extension.domain(), 0.0..=1.0);
            assert_eq!(extension.knots(), &[0.0, 0.0, 1.0, 1.0]);
            assert!(
                Tolerance::DEFAULT.approx_eq(extension.length(Tolerance::DEFAULT).unwrap(), 2.0,)
            );
        }
        assert_point_near(
            extensions[0].control_points()[0].point(),
            Point3::try_new(-0.894_427_190_999_915_9, -1.788_854_381_999_831_7, 0.0).unwrap(),
        );
        assert_eq!(
            extensions[0].control_points()[1].point(),
            curve.evaluate(0.0).unwrap()
        );
        assert_eq!(
            extensions[1].control_points()[0].point(),
            curve.evaluate(1.0).unwrap()
        );
        assert_point_near(
            extensions[1].control_points()[1].point(),
            Point3::try_new(4.788_854_381_999_831_5, 0.105_572_809_000_084_14, 0.0).unwrap(),
        );
    }

    #[test]
    fn separate_natural_extension_is_the_exact_extrapolated_piece() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 2.0, 0.0).unwrap(),
                Point3::try_new(3.0, 1.0, 0.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let mut extensions = curve
            .try_separate_natural_extensions_by_length(
                CurveExtensionSide::End,
                2.0,
                Tolerance::DEFAULT,
            )
            .unwrap();
        let extension = extensions.pop().unwrap();

        assert_eq!(*extension.domain().start(), *curve.domain().end());
        assert!(Tolerance::DEFAULT.approx_eq(extension.length(Tolerance::DEFAULT).unwrap(), 2.0,));
        assert_point_near(
            extension.evaluate(1.0).unwrap(),
            curve.evaluate(1.0).unwrap(),
        );
        let extended = curve
            .try_extended_by_length(CurveExtensionSide::End, 2.0, Tolerance::DEFAULT)
            .unwrap();
        let expected = extended
            .try_trimmed(1.0..=*extended.domain().end())
            .unwrap();
        assert_eq!(extension, expected);
    }

    #[test]
    fn natural_extension_ignores_interior_bound_and_rejects_invalid_inputs() {
        let curve = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0), point(10.0, 0.0)],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        let extended = curve.try_extended_to(0.25..=2.0).unwrap();
        assert_eq!(extended.domain(), 0.0..=2.0);
        assert_eq!(extended.evaluate(2.0).unwrap(), point(20.0, 0.0));

        for interval in [0.0..=1.0, 0.75..=0.25, Real::NAN..=2.0] {
            assert_eq!(
                curve.try_extended_to(interval),
                Err(GeometryError::InvalidCurveExtensionInterval)
            );
        }

        let closed = NurbsCurve::try_control_point_curve_with_closure(
            1,
            vec![
                point(0.0, 0.0),
                point(2.0, 0.0),
                point(2.0, 2.0),
                point(0.0, 2.0),
            ],
            ControlPointCurveClosure::Sharp,
        )
        .unwrap();
        assert_eq!(
            closed.try_extended_to(-1.0..=5.0),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        assert_eq!(
            closed.try_extended_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        assert_eq!(
            closed
                .try_merged_naturally_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT,),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        assert_eq!(
            closed.try_extended_linearly_by_length(
                CurveExtensionSide::End,
                1.0,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        assert_eq!(
            closed.try_merged_linearly_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT,),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        assert_eq!(
            closed.try_extended_circularly_by_length(
                CurveExtensionSide::End,
                1.0,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        assert_eq!(
            closed.try_merged_circularly_by_length(
                CurveExtensionSide::End,
                1.0,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        assert_eq!(
            closed.try_joined_circularly_by_length(
                CurveExtensionSide::End,
                1.0,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        assert_eq!(
            closed.try_separate_circular_extensions_by_length(
                CurveExtensionSide::End,
                1.0,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        assert_eq!(
            closed.try_joined_linearly_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT,),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        assert_eq!(
            closed
                .try_joined_naturally_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT,),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        assert_eq!(
            closed.try_joined_smoothly_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT,),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        assert_eq!(
            closed.try_separate_linear_extensions_by_length(
                CurveExtensionSide::End,
                1.0,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        assert_eq!(
            closed.try_separate_natural_extensions_by_length(
                CurveExtensionSide::End,
                1.0,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        assert_eq!(
            closed.try_separate_smooth_extensions_by_length(
                CurveExtensionSide::End,
                1.0,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::CurveExtensionMustBeOpen)
        );
        for length in [0.0, -1.0, Real::NAN] {
            assert_eq!(
                curve.try_extended_by_length(CurveExtensionSide::End, length, Tolerance::DEFAULT),
                Err(GeometryError::InvalidCurveExtensionLength)
            );
            assert_eq!(
                curve.try_merged_naturally_by_length(
                    CurveExtensionSide::End,
                    length,
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidCurveExtensionLength)
            );
            assert_eq!(
                curve.try_extended_linearly_by_length(
                    CurveExtensionSide::End,
                    length,
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidCurveExtensionLength)
            );
            assert_eq!(
                curve.try_merged_linearly_by_length(
                    CurveExtensionSide::End,
                    length,
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidCurveExtensionLength)
            );
            assert_eq!(
                curve.try_extended_circularly_by_length(
                    CurveExtensionSide::End,
                    length,
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidCurveExtensionLength)
            );
            assert_eq!(
                curve.try_merged_circularly_by_length(
                    CurveExtensionSide::End,
                    length,
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidCurveExtensionLength)
            );
            assert_eq!(
                curve.try_joined_circularly_by_length(
                    CurveExtensionSide::End,
                    length,
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidCurveExtensionLength)
            );
            assert_eq!(
                curve.try_separate_circular_extensions_by_length(
                    CurveExtensionSide::End,
                    length,
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidCurveExtensionLength)
            );
            assert_eq!(
                curve.try_joined_linearly_by_length(
                    CurveExtensionSide::End,
                    length,
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidCurveExtensionLength)
            );
            assert_eq!(
                curve.try_joined_naturally_by_length(
                    CurveExtensionSide::End,
                    length,
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidCurveExtensionLength)
            );
            assert_eq!(
                curve.try_joined_smoothly_by_length(
                    CurveExtensionSide::End,
                    length,
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidCurveExtensionLength)
            );
            assert_eq!(
                curve.try_separate_linear_extensions_by_length(
                    CurveExtensionSide::End,
                    length,
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidCurveExtensionLength)
            );
            assert_eq!(
                curve.try_separate_natural_extensions_by_length(
                    CurveExtensionSide::End,
                    length,
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidCurveExtensionLength)
            );
            assert_eq!(
                curve.try_separate_smooth_extensions_by_length(
                    CurveExtensionSide::End,
                    length,
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidCurveExtensionLength)
            );
        }
    }

    #[test]
    fn directed_subcurves_reverse_open_curves_and_cross_closed_seams() {
        let open = NurbsCurve::try_new_rational(
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
        assert_eq!(
            open.try_subcurve(0.25, 1.6).unwrap(),
            open.try_trimmed(0.25..=1.6).unwrap()
        );
        let reversed = open.try_subcurve(1.6, 0.25).unwrap();
        assert_eq!(reversed.domain(), -1.6..=-0.25);
        assert_point_near(
            reversed.evaluate(-1.6).unwrap(),
            open.evaluate(1.6).unwrap(),
        );
        assert_point_near(
            reversed.evaluate(-0.25).unwrap(),
            open.evaluate(0.25).unwrap(),
        );

        let closed = NurbsCurve::try_control_point_curve_with_closure(
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
        let domain = closed.domain();
        let period = *domain.end() - *domain.start();
        let start = closed.parameter_at(0.8).unwrap();
        let end = closed.parameter_at(0.2).unwrap();
        let wrapped = closed.try_subcurve(start, end).unwrap();
        assert_eq!(wrapped.domain(), start..=end + period);
        assert!(!wrapped.is_closed().unwrap());
        for sample in 0..=32 {
            let parameter = start + (end + period - start) * sample as Real / 32.0;
            let source_parameter = if parameter <= *domain.end() {
                parameter
            } else {
                parameter - period
            };
            assert_point_near(
                wrapped.evaluate(parameter).unwrap(),
                closed.evaluate(source_parameter).unwrap(),
            );
        }
        assert_eq!(
            closed.try_subcurve(*domain.end(), end).unwrap(),
            closed.try_trimmed(*domain.start()..=end).unwrap()
        );
        assert_eq!(
            closed.try_subcurve(start, *domain.start()).unwrap(),
            closed.try_trimmed(start..=*domain.end()).unwrap()
        );

        for (start, end) in [
            (0.5, 0.5),
            (Real::NAN, 0.5),
            (*open.domain().start() - 1.0, 0.5),
        ] {
            assert_eq!(
                open.try_subcurve(start, end),
                Err(GeometryError::InvalidCurveTrimInterval)
            );
        }
    }

    #[test]
    fn splitting_at_multiple_parameters_sorts_and_preserves_exact_pieces() {
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
        let pieces = curve.try_split_at_parameters(&[1.6, 0.25, 0.8]).unwrap();
        assert_eq!(pieces.len(), 4);
        assert_eq!(pieces[0].domain(), 0.0..=0.25);
        assert_eq!(pieces[1].domain(), 0.25..=0.8);
        assert_eq!(pieces[2].domain(), 0.8..=1.6);
        assert_eq!(pieces[3].domain(), 1.6..=2.0);
        for piece in &pieces {
            for sample in 0..=8 {
                let parameter = piece.parameter_at(sample as Real / 8.0).unwrap();
                assert_point_near(
                    piece.evaluate(parameter).unwrap(),
                    curve.evaluate(parameter).unwrap(),
                );
            }
        }
        for parameters in [&[][..], &[0.5, 0.5], &[0.0, 0.5], &[Real::NAN]] {
            assert_eq!(
                curve.try_split_at_parameters(parameters),
                Err(GeometryError::InvalidCurveSplitParameter)
            );
        }

        let closed = NurbsCurve::try_control_point_curve_with_closure(
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
        let parameters = [
            closed.parameter_at(0.8).unwrap(),
            closed.parameter_at(0.2).unwrap(),
            closed.parameter_at(0.55).unwrap(),
        ];
        let pieces = closed.try_split_at_parameters(&parameters).unwrap();
        assert_eq!(pieces.len(), 3);
        let mut sorted = parameters;
        sorted.sort_by(Real::total_cmp);
        let period = *closed.domain().end() - *closed.domain().start();
        assert_eq!(pieces[0].domain(), sorted[0]..=sorted[1]);
        assert_eq!(pieces[1].domain(), sorted[1]..=sorted[2]);
        assert_eq!(pieces[2].domain(), sorted[2]..=sorted[0] + period);
        let relocated = closed.try_split_at_parameters(&[sorted[1]]).unwrap();
        assert_eq!(relocated.len(), 1);
        assert_eq!(relocated[0].domain(), sorted[1]..=sorted[1] + period);
        assert!(!relocated[0].is_periodic());
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
