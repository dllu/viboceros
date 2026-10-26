use std::ops::RangeInclusive;
mod analytic;
mod arc_length;
mod arc_length_from;
mod soft_edit;
pub use soft_edit::CurveSoftEdit;
mod boundary_extension;
mod circularity;
mod conics;
pub use boundary_extension::CurveExtensionOperations;
pub(crate) use boundary_extension::with_portable_extension_operations;
mod closest_point;
pub(crate) use closest_point::PreparedCurveClosest;
mod decompose;
mod discontinuities;
mod edit_point_insertion;
mod ellipticity;
pub use ellipticity::ConicFoci;
mod evaluate;
pub(crate) mod exact;
mod grip_deletion;
mod grips;
mod integration_frame;
mod knot_reduction;
pub use knot_reduction::{KnotReduction, MAX_KNOT_REDUCTION_GROUPS};
mod linear_trim;
mod parameter_frame;
pub(crate) mod periodic;
mod poles;
mod sampling;
mod smoothing;
pub use sampling::{NurbsCurveParameterSampler, NurbsCurveSamplingSpan};
mod weights;
use weights::change_bezier_end_weights;
pub(crate) use weights::rescale_controls;

use faer::{Mat, prelude::*};

use crate::{
    AffineTransform3, BoundingBox3, Brep, CircularArc3, Frame3, GeometryError, NurbsSurface,
    Point3, Polyline3, Real, Tolerance, Vector3, require_finite,
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

mod intersection_backend;
pub use intersection_backend::CurveCurveIntersectionParameters;

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
    rational_representation: bool,
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
        let rational_representation = control_points.iter().any(|p| p.weight() != 1.);
        Ok(Self {
            degree,
            control_points,
            knots,
            rational,
            rational_representation,
        })
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

    /// Whether the weights vary, independently of an explicit weight coordinate.
    /// Use `has_rational_representation` to inspect retained NURBS storage.
    #[inline]
    pub const fn is_rational(&self) -> bool {
        self.rational
    }

    /// Whether the NURBS definition retains a homogeneous weight coordinate,
    /// including imported unit weights. This is distinct from varying weights.
    pub const fn has_rational_representation(&self) -> bool {
        self.rational_representation
    }
    /// Retain an explicit rational definition without changing its geometry.
    pub fn with_rational_representation(mut self) -> Self {
        self.rational_representation = true;
        self
    }
    fn preserving_representation(mut self, source: &Self) -> Self {
        self.rational_representation |= source.rational_representation;
        self
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
                .with_opennurbs_outer_knots()
                .preserving_representation(self),
        )
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
        let knots = degree_change_knots(source.degree, desired_degree, deformable, &source.knots)?;
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
        self.try_make_periodic_with_solver(smooth, &mut portable_periodic_interpolation)
    }

    /// Retain closure, knot and homogeneous seam policy while delegating the
    /// smooth periodic basis solve. Parameters include the final domain endpoint
    /// without a repeated target row; the solver returns unique homogeneous controls.
    /// Its second argument identifies the control index of the first Greville
    /// site, allowing a backend to align periodic columns for banded solving.
    pub fn try_make_periodic_with_solver<E: From<GeometryError>>(
        &self,
        smooth: bool,
        solve: &mut impl FnMut(&crate::NurbsInterpolationSystem<'_>, usize) -> Result<Vec<Real>, E>,
    ) -> Result<Self, E> {
        if self.is_periodic() {
            return Ok(self.clone());
        }
        if self.degree < 2 {
            return Err(GeometryError::PeriodicNurbsDegreeTooLow.into());
        }
        if !self.is_closed()? {
            return Err(GeometryError::PeriodicCurveMustBeClosed.into());
        }
        self.try_make_periodic_assuming_closed_with_solver(smooth, solve)
    }

    pub(crate) fn try_make_periodic_assuming_closed_with_solver<E: From<GeometryError>>(
        &self,
        smooth: bool,
        solve: &mut impl FnMut(&crate::NurbsInterpolationSystem<'_>, usize) -> Result<Vec<Real>, E>,
    ) -> Result<Self, E> {
        if self.is_periodic() {
            return Ok(self.clone());
        }
        if self.degree < 2 {
            return Err(GeometryError::PeriodicNurbsDegreeTooLow.into());
        }
        if smooth {
            self.make_periodic_smooth(solve)
        } else {
            Ok(self.make_periodic_with_minimal_control_change()?)
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

    fn make_periodic_smooth<E: From<GeometryError>>(
        &self,
        solve: &mut impl FnMut(&crate::NurbsInterpolationSystem<'_>, usize) -> Result<Vec<Real>, E>,
    ) -> Result<Self, E> {
        Ok(periodic::smooth_curves(std::slice::from_ref(self), solve)?.remove(0))
    }

    pub(crate) fn evaluate_scaled_homogeneous(
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
        let sampler = self.parameter_sampler()?;
        let first_interior = sampler.evaluate(1.0 / 3.0)?;
        let second_interior = sampler.evaluate(2.0 / 3.0)?;
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
    /// Defining points emitted by Rhino's ExtractPt command. Closed seam grips
    /// are shared using point coincidence; querying this list does not change
    /// either stored endpoint control. Periodic curves omit repeated tail CVs.
    pub fn extract_point_locations(&self) -> Result<Vec<Point3>, GeometryError> {
        Ok(self
            .control_points
            .iter()
            .take(self.grip_count()?)
            .map(|control| control.point())
            .collect())
    }

    /// Number of distinct editable controls, excluding repeated seam controls.
    /// Closed seam grips and ExtractPt share point-coincident endpoint controls;
    /// the full control net remains available through `control_points`.
    pub fn grip_count(&self) -> Result<usize, GeometryError> {
        let count = self.control_points.len();
        if self.is_periodic() {
            Ok(count - self.degree)
        } else if self.is_closed()?
            && count > 1
            && curve_points_coincident(
                self.control_points[0].point,
                self.control_points[count - 1].point,
            )
        {
            Ok(count - 1)
        } else {
            Ok(count)
        }
    }

    /// Fits Rhino's extracted degree-one control polygon through this curve's
    /// Euclidean control locations.
    ///
    /// Periodic curves use the domain-aligned Greville window returned by
    /// `Curve.ControlPolygon`, retaining one repeated endpoint to close the
    /// polyline. Unlike `ExtractPt`, this can therefore rotate the first
    /// output point away from raw control index zero.
    pub fn control_polygon(&self, tolerance: Tolerance) -> Result<Polyline3, GeometryError> {
        let range = self.control_polygon_range();
        let mut points = self.control_points[range]
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

    /// Raw control indices for the domain-aligned control polygon. Periodic
    /// curves retain one closing control; other curves retain their full net.
    pub fn control_polygon_range(&self) -> std::ops::Range<usize> {
        let (start, end) = control_polygon_range(
            self.degree,
            self.control_points.len(),
            &self.knots,
            self.is_periodic(),
        );
        start..end
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
    ///
    /// The returned native parameter can round onto a coarse floating-point
    /// grid. Use [`Self::parameter_sampler`] to sample geometry directly.
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

        let (coordinate_scale, distance_tolerance) =
            intersection_backend::distance_tolerance(self, other, tolerance);
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
        Ok(intersection_backend::finalize(
            intersections,
            overlaps,
            distance_tolerance,
        ))
    }

    /// Reverses direction by reversing the controls and negating the full
    /// knot vector. A domain `[a, b]` therefore becomes `[-b, -a]`, matching
    /// the OpenNURBS/Rhino convention.
    pub fn reversed(&self) -> Result<Self, GeometryError> {
        let control_points = self.control_points.iter().rev().copied().collect();
        let knots = self.knots.iter().rev().map(|knot| -*knot).collect();
        Ok(Self::try_new_rational(self.degree, control_points, knots)?
            .preserving_representation(self))
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
        self.try_change_closed_seam_with_operations(
            parameter,
            &mut |curve, knot, target| curve.try_insert_knot(knot, target),
            &mut |curve, knot, periodic| curve.try_split_with_periodic_topology(knot, periodic),
        )
    }

    /// Apply the existing seam policy with external exact refinement and split
    /// operations. The split receives the validated periodic-topology policy.
    pub fn try_change_closed_seam_with_operations<E: From<GeometryError>>(
        &self,
        parameter: Real,
        refine: &mut impl FnMut(&Self, Real, usize) -> Result<Self, E>,
        split: &mut impl FnMut(&Self, Real, bool) -> Result<(Self, Self), E>,
    ) -> Result<Self, E> {
        require_finite([parameter], "curve seam parameter")?;
        if !self.is_closed()? {
            return Err(GeometryError::CurveSeamMustBeClosed.into());
        }
        let domain = self.domain();
        if !domain.contains(&parameter) {
            let wrapped = crate::parameter::wrapped_parameter(parameter, &domain)?;
            // OpenNURBS' periodic seam mover receives the original parameter
            // and falls back to clamped split/append outside the old interval.
            let relocated = if wrapped > *domain.start() && wrapped < *domain.end() {
                self.change_non_periodic_seam_with_operation(
                    wrapped,
                    *domain.start(),
                    *domain.end(),
                    self.is_periodic(),
                    split,
                )?
            } else {
                self.clone()
            };
            return Ok(relocated.try_reparameterized(
                parameter..=crate::parameter::shifted_parameter(parameter, &domain)?,
            )?);
        }
        self.change_closed_seam_with_operations(parameter, self.is_periodic(), refine, split)
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
        self.change_closed_seam_with_operations(
            parameter,
            periodic,
            &mut |curve, knot, target| curve.try_insert_knot(knot, target),
            &mut |curve, knot, periodic| curve.try_split_with_periodic_topology(knot, periodic),
        )
    }

    fn change_closed_seam_with_operations<E: From<GeometryError>>(
        &self,
        parameter: Real,
        periodic: bool,
        refine: &mut impl FnMut(&Self, Real, usize) -> Result<Self, E>,
        split: &mut impl FnMut(&Self, Real, bool) -> Result<(Self, Self), E>,
    ) -> Result<Self, E> {
        let domain = self.domain();
        let start = *domain.start();
        let end = *domain.end();
        if parameter == start {
            return Ok(self.clone());
        }
        if parameter == end {
            return Ok(self.translate_parameterization_by_period(start, end, 1)?);
        }
        if periodic && self.knot_multiplicity_unchecked(parameter) <= 1 {
            Ok(Self::periodic_seam_from_refined(
                refine(self, parameter, 1)?,
                parameter,
                start,
                end,
            )?)
        } else {
            self.change_non_periodic_seam_with_operation(parameter, start, end, periodic, split)
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

    pub(crate) fn periodic_seam_from_refined(
        refined: Self,
        parameter: Real,
        domain_start: Real,
        domain_end: Real,
    ) -> Result<Self, GeometryError> {
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

    fn change_non_periodic_seam_with_operation<E: From<GeometryError>>(
        &self,
        parameter: Real,
        old_start: Real,
        old_end: Real,
        refine_periodic_topology: bool,
        split: &mut impl FnMut(&Self, Real, bool) -> Result<(Self, Self), E>,
    ) -> Result<Self, E> {
        let (left, right) = split(self, parameter, refine_periodic_topology)?;
        Ok(Self::closed_seam_from_split(
            left, right, old_start, old_end,
        )?)
    }

    pub(crate) fn closed_seam_from_split(
        left: Self,
        right: Self,
        old_start: Real,
        old_end: Real,
    ) -> Result<Self, GeometryError> {
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
        let knot = self.knot_removal_candidate(parameter)?;
        self.try_remove_knot_value(knot)
    }

    /// Select the removable knot using the command's model-space distance
    /// and tie policy, without constructing new controls.
    pub fn knot_removal_candidate(&self, parameter: Real) -> Result<Real, GeometryError> {
        self.validate_parameter(parameter)?;
        if self.is_periodic() {
            return Err(GeometryError::PeriodicKnotRemovalUnsupported { direction: "curve" });
        }
        let knot_index = self.closest_active_knot_index_by_curve_point(parameter)?;
        removable_knot_at_index(self.degree, &self.knots, knot_index, parameter)
    }

    /// Inserts one control point while retaining the source knot chart.
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

    /// InsertControlPoint rebuilds uniform charts using their source spacing.
    /// Uniform endpoint picks may duplicate an end control; midpoint picks use the
    /// neighboring Greville interval. The parameter-preserving API above is
    /// retained for callers that explicitly manage their knot vector.
    pub fn try_insert_command_control_point(
        &self,
        parameter: Real,
        midpoint: bool,
    ) -> Result<Self, GeometryError> {
        self.validate_parameter(parameter)?;
        if !midpoint
            && self.command_uniform_step().is_none()
            && self.knots.iter().filter(|&&k| k == parameter).count() >= self.degree
        {
            return Err(GeometryError::NoControlPointInsertionInterval { parameter });
        }
        let endpoint = !self.is_periodic()
            && (parameter == *self.domain().start() || parameter == *self.domain().end());
        let mut result = self.try_insert_control_point(parameter, midpoint || endpoint)?;
        if endpoint && !midpoint {
            let (_, upper, _) = self.control_point_insertion_interval(parameter, true)?;
            result.control_points[upper] = WeightedPoint3::try_new(self.evaluate(parameter)?, 1.0)?;
        }
        if !result.is_periodic()
            && let Some(step) = self.command_uniform_step()
        {
            let end = (result.control_points.len() - result.degree) as Real * step;
            result = result.try_make_uniform()?.try_reparameterized(0.0..=end)?;
        }
        Ok(result)
    }

    /// Add an outside control half an end-polygon interval beyond the end.
    /// The command rebuilds the full net as clamped and non-rational, including
    /// repeated controls from a previously periodic curve.
    pub fn try_extend_control_point_net(&self, at_end: bool) -> Result<Self, GeometryError> {
        let n = self.control_points.len();
        let (end, neighbor) = if at_end { (n - 1, n - 2) } else { (0, 1) };
        let end_point = self.control_points[end].point();
        let delta = self.control_points[neighbor]
            .point()
            .vector_to(end_point)?
            .scaled(0.5)?;
        let mut controls = self
            .control_points
            .iter()
            .map(|c| c.point())
            .collect::<Vec<_>>();
        controls.insert(if at_end { n } else { 0 }, end_point.translated(delta)?);
        Self::try_clamped_uniform(self.degree, controls)?.try_reparameterized(self.domain())
    }

    fn command_uniform_step(&self) -> Option<Real> {
        // Match OpenNURBS ON_IsKnotVectorUniform: the spacing-relative
        // tolerance is independent of the parameter origin. Include usable
        // padding at any unclamped end (the full outer slots are artificial).
        let count = self.control_points.len();
        let first = if self.knots[1] == self.knots[self.degree] {
            self.degree
        } else {
            1
        };
        let last = if self.knots[count] == self.knots[self.knots.len() - 2] {
            count
        } else {
            self.knots.len() - 2
        };
        let step = self.knots[self.degree + 1] - self.knots[self.degree];
        (step > 0.0
            && self.knots[first..=last].windows(2).all(|p| {
                let delta = p[1] - p[0];
                delta > 0.0 && (delta - step).abs() <= Real::EPSILON.sqrt() * step
            }))
        .then_some(step)
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
        self.remove_control_point(index, true)
    }

    /// RemoveControlPoint retains homogeneous weights, unlike deleting grips.
    /// Closed quadratic seams receive periodic control layouts.
    pub fn try_remove_control_point_preserving_weights(
        &self,
        index: usize,
    ) -> Result<Self, GeometryError> {
        if self.is_periodic() && self.control_points.len() - self.degree > self.degree + 1 {
            let mut controls =
                self.control_points[..self.control_points.len() - self.degree].to_vec();
            if index >= controls.len() {
                return Err(GeometryError::InvalidControlPointIndex {
                    index,
                    count: controls.len(),
                });
            }
            controls.remove(index);
            controls.extend_from_within(..self.degree);
            let target = (index + self.degree.div_ceil(2))
                .clamp(self.degree + 1, self.control_points.len() - 1);
            let mut knots = self.knots.clone();
            if self.degree.is_multiple_of(2)
                && target > self.degree + 1
                && target < self.control_points.len() - 1
            {
                knots[target - 1] = stable_knot_mean(&[knots[target - 1], knots[target]])?;
            }
            knots.remove(target);
            let knots = periodic_knots_preserving_active(self.degree, controls.len(), &knots)?;
            return Self::try_new_rational(self.degree, controls, knots);
        }
        if !self.is_periodic() && self.is_closed()? && index == 0 && self.degree == 2 {
            let n = self.control_points.len();
            if n < 4 {
                return Err(GeometryError::InsufficientControlPoints {
                    degree: self.degree,
                    required: 4,
                    actual: n,
                });
            }
            let mut controls = self.control_points[1..n - 1].to_vec();
            controls.rotate_right(1);
            controls.extend_from_within(..self.degree);
            let knots = periodic_knots_preserving_active(self.degree, controls.len(), &self.knots)?;
            return Self::try_new_rational(self.degree, controls, knots);
        }
        let result = self.remove_control_point(index, false)?;
        if self.is_periodic() {
            Ok(result)
        } else {
            result.try_reparameterized(0.0..=(*result.domain().end() - *result.domain().start()))
        }
    }

    fn remove_control_point(
        &self,
        index: usize,
        normalize_weights: bool,
    ) -> Result<Self, GeometryError> {
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
                .remove_control_point(index, normalize_weights);
        }

        let mut controls = self.control_points.clone();
        controls.remove(index);
        if normalize_weights {
            normalize_control_point_removal_end_weights(&mut controls)?;
        }

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
        let removals = self.multiple_knot_removal_candidates(
            remove_fully_multiple_knots,
            maximum_kink_angle_radians,
        )?;
        let removed = removals.iter().map(|(_, count)| *count).sum();
        if removed == 0 {
            return Ok((self.clone(), 0));
        }
        Ok((self.try_remove_multiple_knot_groups(&removals)?, removed))
    }

    /// Choose ascending interior knot groups and removal counts using the
    /// command's strict angle filter, without fitting new controls.
    pub fn multiple_knot_removal_candidates(
        &self,
        remove_fully_multiple_knots: bool,
        maximum_kink_angle_radians: Real,
    ) -> Result<Vec<(Real, usize)>, GeometryError> {
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

        Ok(removals)
    }

    /// Remove this exact interior knot value, clamping open ends before fitting.
    /// Callers that choose a marker need not repeat a closest-point search.
    pub fn try_remove_knot_value(&self, knot: Real) -> Result<Self, GeometryError> {
        let source = self.clamped_to_active_domain()?;
        let knots = knot_removal_knots(source.degree, &source.knots, knot)?;
        source.interpolate_homogeneous_in_basis(
            source.degree,
            knots,
            GeometryError::KnotRemovalSolveFailed,
        )
    }

    pub(crate) fn interior_knot_groups(&self) -> Vec<(Real, usize)> {
        basis_interior_knot_groups(self.degree, &self.knots)
    }

    pub(crate) fn kink_angle_at(&self, knot: Real) -> Result<Real, GeometryError> {
        let incoming = match self.tangent_at_on_side(knot, crate::ParameterSide::Left) {
            Ok(tangent) => tangent,
            Err(GeometryError::Degenerate { .. }) => return Ok(std::f64::consts::PI),
            Err(error) => return Err(error),
        };
        let outgoing = match self.tangent_at_on_side(knot, crate::ParameterSide::Right) {
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

    /// Interior full-multiplicity knots whose one-sided tangent angle exceeds
    /// the angular tolerance, in increasing parameter order.
    pub fn full_multiplicity_kink_parameters(
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
            let knots =
                knot_group_removal_knots(result.degree, &result.knots, *knot, *removal_count)?;
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

    /// Restore the existing periodic padding and OpenNURBS outer-knot
    /// convention after an external exact refinement. No controls are blended.
    pub fn restore_knot_insertion_topology(
        &self,
        refined: Self,
        parameter: Real,
        target: usize,
    ) -> Result<Self, GeometryError> {
        self.restore_knot_insertion_topology_with_periodic(
            refined,
            parameter,
            target,
            self.is_periodic(),
        )
    }

    pub(crate) fn restore_knot_insertion_topology_with_periodic(
        &self,
        mut refined: Self,
        parameter: Real,
        target: usize,
        periodic: bool,
    ) -> Result<Self, GeometryError> {
        self.validate_parameter(parameter)?;
        if refined.degree != self.degree || refined.domain() != self.domain() {
            return Err(GeometryError::InvalidControlNet {
                context: "refinement changed curve degree or active domain",
            });
        }
        if target == 0 || target > self.degree + 1 {
            return Err(GeometryError::InvalidKnotMultiplicity {
                actual: target,
                maximum: self.degree + 1,
            });
        }
        let domain = self.domain();
        if self.knot_multiplicity_unchecked(parameter) < target
            && periodic
            && target <= self.degree
            && parameter > *domain.start()
            && parameter < *domain.end()
        {
            refined = refined
                .restore_periodic_after_knot_insertion(self.find_span(parameter) - self.degree)?;
        }
        Ok(refined.with_opennurbs_outer_knots())
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

        let domain = self.domain();
        // A full-order endpoint group can be shifted into exterior padding,
        // leaving inactive controls. Its multiplicity alone does not mean the
        // active endpoint is clamped.
        let current_multiplicity = self.knot_multiplicity_unchecked(parameter);
        let full_endpoint = target_multiplicity == maximum
            && (parameter == *domain.start() || parameter == *domain.end());
        if current_multiplicity >= target_multiplicity && !full_endpoint {
            return Ok(self.clone().with_opennurbs_outer_knots());
        }
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

        let mut refined = self.clone();
        while refined.knot_multiplicity_unchecked(parameter) < target_multiplicity {
            refined = refined.insert_knot_once(parameter)?;
        }
        self.restore_knot_insertion_topology_with_periodic(
            refined,
            parameter,
            target_multiplicity,
            restore_periodic_topology,
        )
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
        let parameters = self.checked_split_parameters(parameters)?;

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

    fn checked_split_parameters(&self, parameters: &[Real]) -> Result<Vec<Real>, GeometryError> {
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

        Ok(parameters)
    }

    /// Use the existing cut validation and cyclic ordering with an external
    /// exact interval extractor. Closed seam pieces retain the shared append policy.
    pub fn try_split_at_parameters_with_extractor<E: From<GeometryError>>(
        &self,
        parameters: &[Real],
        trim: &mut impl FnMut(&Self, RangeInclusive<Real>) -> Result<Self, E>,
    ) -> Result<Vec<Self>, E> {
        let cuts = self.checked_split_parameters(parameters)?;
        let domain = self.domain();
        if self.is_closed()? {
            if cuts.len() == 1 {
                return Ok(vec![self.try_closed_subcurve_across_seam_with_extractor(
                    cuts[0], cuts[0], trim,
                )?]);
            }
            let mut pieces = cuts
                .windows(2)
                .map(|pair| self.try_subcurve_with_extractor(pair[0], pair[1], trim))
                .collect::<Result<Vec<_>, E>>()?;
            pieces.push(self.try_subcurve_with_extractor(*cuts.last().unwrap(), cuts[0], trim)?);
            Ok(pieces)
        } else {
            let mut bounds = Vec::with_capacity(cuts.len() + 2);
            bounds.push(*domain.start());
            bounds.extend(cuts);
            bounds.push(*domain.end());
            bounds
                .windows(2)
                .map(|pair| trim(self, pair[0]..=pair[1]))
                .collect()
        }
    }

    pub(crate) fn try_split_with_periodic_topology(
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
        Ok((
            left.preserving_representation(self),
            right.preserving_representation(self),
        ))
    }

    /// Extracts an exact subcurve while retaining the source parameter values.
    /// Partial trims are clamped at both active ends. Trimming to the exact
    /// existing domain is a no-op, preserving periodic form.
    pub fn try_trimmed(&self, interval: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        let start = *interval.start();
        let end = *interval.end();
        let domain = self.domain();
        crate::parameter::check_trim_interval(&interval, domain.clone())?;
        if start == *domain.start() && end == *domain.end() {
            return Ok(self.clone());
        }
        if let Some(line) = self.try_trim_polynomial_line(&interval)? {
            return Ok(line.preserving_representation(self));
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

    /// Smoothly extrapolates the selected end or ends by the requested arc
    /// length. `Both` applies the full length independently at each end.
    pub fn try_extended_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        self.try_extended_by_length_with_operations(
            side,
            length,
            tolerance,
            &mut |curve, interval| curve.try_extended_to(interval),
            &mut |curve, interval| curve.try_trimmed(interval),
        )
    }

    /// Shared arc-length bracketing with delegated exact extension and extraction.
    pub fn try_extended_by_length_with_operations<E: From<GeometryError>>(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
        extend: &mut impl FnMut(&Self, RangeInclusive<Real>) -> Result<Self, E>,
        trim: &mut impl FnMut(&Self, RangeInclusive<Real>) -> Result<Self, E>,
    ) -> Result<Self, E> {
        if !length.is_finite() || length <= 0.0 {
            return Err(GeometryError::InvalidCurveExtensionLength.into());
        }
        if self.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen.into());
        }

        let domain = self.domain();
        let start = match side {
            CurveExtensionSide::Start | CurveExtensionSide::Both => self
                .extension_parameter_by_length_with_operations(
                    true, length, tolerance, extend, trim,
                )?,
            CurveExtensionSide::End => *domain.start(),
        };
        let end = match side {
            CurveExtensionSide::End | CurveExtensionSide::Both => self
                .extension_parameter_by_length_with_operations(
                    false, length, tolerance, extend, trim,
                )?,
            CurveExtensionSide::Start => *domain.end(),
        };
        extend(self, start..=end)
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
        self.try_merged_naturally_by_length_with_extender(
            side,
            length,
            tolerance,
            &mut |source, side, length, tolerance| {
                source.try_extended_by_length(side, length, tolerance)
            },
        )
    }

    /// Preserve natural line/arc policy with a delegated smooth extension backend.
    pub fn try_merged_naturally_by_length_with_extender<E: From<GeometryError>>(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
        smooth: &mut impl FnMut(&Self, CurveExtensionSide, Real, Tolerance) -> Result<Self, E>,
    ) -> Result<Self, E> {
        self.validate_length_extension(length)?;
        if self.degree == 1 || self.is_linear_at_zero_tolerance()? {
            return self
                .try_merged_linearly_by_length(side, length, tolerance)
                .map_err(E::from);
        }
        if self.try_canonical_circular_arc(tolerance)?.is_some() {
            return self
                .try_merged_circularly_by_length(side, length, tolerance)
                .map_err(E::from);
        }
        smooth(self, side, length, tolerance)
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
        self.try_joined_naturally_by_length_with_extender(
            side,
            length,
            tolerance,
            &mut |source, side, length, tolerance| {
                source.try_joined_smoothly_by_length(side, length, tolerance)
            },
        )
    }

    /// Preserve natural line/arc policy with a delegated smooth extension backend.
    pub fn try_joined_naturally_by_length_with_extender<E: From<GeometryError>>(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
        smooth: &mut impl FnMut(&Self, CurveExtensionSide, Real, Tolerance) -> Result<Self, E>,
    ) -> Result<Self, E> {
        self.validate_length_extension(length)?;
        if self.degree == 1 || self.is_linear_at_zero_tolerance()? {
            return self
                .try_joined_linearly_by_length(side, length, tolerance)
                .map_err(E::from);
        }
        if self.try_canonical_circular_arc(tolerance)?.is_some() {
            return self
                .try_joined_circularly_by_length(side, length, tolerance)
                .map_err(E::from);
        }
        smooth(self, side, length, tolerance)
    }

    /// Joins smooth NURBS extrapolation pieces to the unchanged source with
    /// explicit full-multiplicity seams.
    pub fn try_joined_smoothly_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        self.try_joined_smoothly_by_length_with_operations(
            side,
            length,
            tolerance,
            &mut |curve, interval| curve.try_extended_to(interval),
            &mut |curve, interval| curve.try_trimmed(interval),
        )
    }

    /// Retain the existing seam/representation policy with delegated extrapolation and trim.
    pub fn try_joined_smoothly_by_length_with_operations<E: From<GeometryError>>(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
        extend: &mut impl FnMut(&Self, RangeInclusive<Real>) -> Result<Self, E>,
        trim: &mut impl FnMut(&Self, RangeInclusive<Real>) -> Result<Self, E>,
    ) -> Result<Self, E> {
        if self.degree == 1 {
            return self
                .try_joined_linearly_by_length(side, length, tolerance)
                .map_err(E::from);
        }
        if !length.is_finite() || length <= 0.0 {
            return Err(GeometryError::InvalidCurveExtensionLength.into());
        }
        if self.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen.into());
        }

        let domain = self.domain();
        let start_extension =
            if matches!(side, CurveExtensionSide::Start | CurveExtensionSide::Both) {
                let parameter = self.extension_parameter_by_length_with_operations(
                    true, length, tolerance, extend, trim,
                )?;
                Some(self.extension_segment_with_operations(true, parameter, extend, trim)?)
            } else {
                None
            };
        let end_extension = if matches!(side, CurveExtensionSide::End | CurveExtensionSide::Both) {
            let parameter = self.extension_parameter_by_length_with_operations(
                false, length, tolerance, extend, trim,
            )?;
            Some(self.extension_segment_with_operations(false, parameter, extend, trim)?)
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
        self.try_separate_natural_extensions_by_length_with_extender(
            side,
            length,
            tolerance,
            &mut |source, side, length, tolerance| {
                source.try_separate_smooth_extensions_by_length(side, length, tolerance)
            },
        )
    }

    /// Preserve natural line/arc policy with a delegated smooth extension backend.
    pub fn try_separate_natural_extensions_by_length_with_extender<E: From<GeometryError>>(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
        smooth: &mut impl FnMut(&Self, CurveExtensionSide, Real, Tolerance) -> Result<Vec<Self>, E>,
    ) -> Result<Vec<Self>, E> {
        self.validate_length_extension(length)?;
        if self.degree == 1 || self.is_linear_at_zero_tolerance()? {
            return self
                .try_separate_linear_extensions_by_length(side, length, tolerance)
                .map_err(E::from);
        }
        if self.try_canonical_circular_arc(tolerance)?.is_some() {
            return self
                .try_separate_circular_extensions_by_length(side, length, tolerance)
                .map_err(E::from);
        }
        smooth(self, side, length, tolerance)
    }

    /// Creates independent smooth NURBS extrapolation pieces without changing
    /// the source. Degree-one inputs use standalone line representation.
    pub fn try_separate_smooth_extensions_by_length(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Vec<Self>, GeometryError> {
        self.try_separate_smooth_extensions_by_length_with_operations(
            side,
            length,
            tolerance,
            &mut |curve, interval| curve.try_extended_to(interval),
            &mut |curve, interval| curve.try_trimmed(interval),
        )
    }

    /// Retain the existing seam/representation policy with delegated extrapolation and trim.
    pub fn try_separate_smooth_extensions_by_length_with_operations<E: From<GeometryError>>(
        &self,
        side: CurveExtensionSide,
        length: Real,
        tolerance: Tolerance,
        extend: &mut impl FnMut(&Self, RangeInclusive<Real>) -> Result<Self, E>,
        trim: &mut impl FnMut(&Self, RangeInclusive<Real>) -> Result<Self, E>,
    ) -> Result<Vec<Self>, E> {
        if self.degree == 1 {
            return self
                .try_separate_linear_extensions_by_length(side, length, tolerance)
                .map_err(E::from);
        }
        if !length.is_finite() || length <= 0.0 {
            return Err(GeometryError::InvalidCurveExtensionLength.into());
        }
        if self.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen.into());
        }

        let mut extensions = Vec::with_capacity(if side == CurveExtensionSide::Both {
            2
        } else {
            1
        });
        if matches!(side, CurveExtensionSide::Start | CurveExtensionSide::Both) {
            let parameter = self.extension_parameter_by_length_with_operations(
                true, length, tolerance, extend, trim,
            )?;
            extensions.push(self.extension_segment_with_operations(true, parameter, extend, trim)?);
        }
        if matches!(side, CurveExtensionSide::End | CurveExtensionSide::Both) {
            let parameter = self.extension_parameter_by_length_with_operations(
                false, length, tolerance, extend, trim,
            )?;
            extensions
                .push(self.extension_segment_with_operations(false, parameter, extend, trim)?);
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

    fn extension_parameter_by_length_with_operations<E: From<GeometryError>>(
        &self,
        at_start: bool,
        length: Real,
        tolerance: Tolerance,
        extend: &mut impl FnMut(&Self, RangeInclusive<Real>) -> Result<Self, E>,
        trim: &mut impl FnMut(&Self, RangeInclusive<Real>) -> Result<Self, E>,
    ) -> Result<Real, E> {
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
                return Err(GeometryError::CurveExtensionLengthDidNotConverge.into());
            }
            let extension =
                self.extension_segment_with_operations(at_start, parameter, extend, trim)?;
            let sampler = ArcLengthSampler::try_new(CurveRef::NurbsCurve(&extension), tolerance)?;
            let extension_length = sampler.total_length();
            if extension_length >= length {
                let distance = if at_start {
                    extension_length - length
                } else {
                    length
                };
                return sampler.parameter_at_distance(distance).map_err(E::from);
            }
            step *= 2.0;
            if !step.is_finite() {
                return Err(GeometryError::CurveExtensionLengthDidNotConverge.into());
            }
        }
        Err(GeometryError::CurveExtensionLengthDidNotConverge.into())
    }

    fn extension_segment_with_operations<E: From<GeometryError>>(
        &self,
        at_start: bool,
        parameter: Real,
        extend: &mut (impl FnMut(&Self, RangeInclusive<Real>) -> Result<Self, E> + ?Sized),
        trim: &mut (impl FnMut(&Self, RangeInclusive<Real>) -> Result<Self, E> + ?Sized),
    ) -> Result<Self, E> {
        let domain = self.domain();
        let (start, end) = (*domain.start(), *domain.end());
        if at_start {
            trim(&extend(self, parameter..=end)?, parameter..=start)
        } else {
            trim(&extend(self, start..=parameter)?, end..=parameter)
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

        // Sampled curvature centers above are a cheap rejection test, not a
        // certificate: higher-degree curves can share all those sampled jets
        // while departing from the circle between them. Do not canonicalize
        // (and thereby replace) their geometry without whole-span bounds.
        if self.circular_radius(tolerance)?.is_none() {
            return Ok(None);
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
        self.try_subcurve_with_extractor(start, end, &mut |curve, interval| {
            curve.try_trimmed(interval)
        })
    }

    /// Retain directed/reversed and closed-seam policies while delegating
    /// increasing intervals to a caller-supplied exact segment extractor.
    pub fn try_subcurve_with_extractor<E: From<GeometryError>>(
        &self,
        start: Real,
        end: Real,
        trim: &mut impl FnMut(&Self, RangeInclusive<Real>) -> Result<Self, E>,
    ) -> Result<Self, E> {
        if !start.is_finite() || !end.is_finite() || start == end {
            return Err(GeometryError::InvalidCurveTrimInterval.into());
        }
        let domain = self.domain();
        let domain_start = *domain.start();
        let domain_end = *domain.end();
        if start < domain_start || start > domain_end || end < domain_start || end > domain_end {
            return Err(GeometryError::InvalidCurveTrimInterval.into());
        }
        if start < end {
            return trim(self, start..=end);
        }
        if !self.is_closed()? {
            return Ok(trim(self, end..=start)?.reversed()?);
        }
        if start == domain_end {
            return trim(self, domain_start..=end);
        }
        if end == domain_start {
            return trim(self, start..=domain_end);
        }

        self.try_closed_subcurve_across_seam_with_extractor(start, end, trim)
    }

    fn try_closed_subcurve_across_seam(
        &self,
        start: Real,
        end: Real,
    ) -> Result<Self, GeometryError> {
        self.try_closed_subcurve_across_seam_with_extractor(start, end, &mut |curve, interval| {
            curve.try_trimmed(interval)
        })
    }

    fn try_closed_subcurve_across_seam_with_extractor<E: From<GeometryError>>(
        &self,
        start: Real,
        end: Real,
        trim: &mut impl FnMut(&Self, RangeInclusive<Real>) -> Result<Self, E>,
    ) -> Result<Self, E> {
        let domain = self.domain();
        let domain_start = *domain.start();
        let domain_end = *domain.end();
        let left = trim(self, start..=domain_end)?;
        let wrapped_end = translate_curve_parameter(end, domain_start, domain_end, 1)?;
        let right =
            trim(self, domain_start..=end)?.try_reparameterized(domain_end..=wrapped_end)?;
        Ok(left.try_append_clamped(&right)?)
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
        Ok(
            Self::try_new_rational(self.degree, control_points, self.knots.clone())?
                .preserving_representation(self),
        )
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
        // Arbitrary full vectors may already have a full-order group next to
        // an outer entry. Duplicating it would create an invalid multiplicity.
        // Preserve that exterior entry when the short OpenNURBS convention
        // cannot represent the complete basis.
        let first = self.knots[1];
        if self.knots[1..]
            .iter()
            .take_while(|knot| **knot == first)
            .count()
            <= self.degree
        {
            self.knots[0] = first;
        }
        let last = self.knots.len() - 1;
        let end = self.knots[last - 1];
        if self.knots[..last]
            .iter()
            .rev()
            .take_while(|knot| **knot == end)
            .count()
            <= self.degree
        {
            self.knots[last] = end;
        }
        self
    }

    /// Circular parameter length for the existing Revolve command convention.
    /// This retains its 65-station radius estimate, not an exact maximum-radius
    /// query or an error bound. It changes no geometry and needs no control net.
    pub fn revolution_parameter_length(
        &self,
        axis_origin: Point3,
        axis_direction: crate::UnitVector3,
        sweep_angle_radians: Real,
    ) -> Result<Real, GeometryError> {
        if !sweep_angle_radians.is_finite()
            || sweep_angle_radians == 0.
            || sweep_angle_radians.abs() > std::f64::consts::TAU
        {
            return Err(GeometryError::InvalidRevolutionSweep);
        }
        let axis_vector = axis_direction.as_vector();
        let mut radius: Real = 0.;
        for index in 0..=64 {
            let point = self.evaluate(self.parameter_at(index as Real / 64.)?)?;
            let offset = axis_origin.vector_to(point)?;
            let center = axis_origin.translated(axis_vector.scaled(offset.dot(axis_vector)?)?)?;
            radius = radius.max(point.distance_to(center)?);
        }
        if radius == 0. {
            return Err(GeometryError::Degenerate {
                context: "revolution profile radius",
            });
        }
        let length = radius * sweep_angle_radians.abs();
        if !length.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "revolution parameter domain",
            });
        }
        Ok(length)
    }

    /// Clamps the active interval without changing its parameterization or locus.
    /// Exterior knots and controls are removed using the existing de Boor
    /// endpoint extraction. The source is unchanged.
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

/// Shared target-basis policy for degree changes on a clamped active interval.
/// This chooses knot structure only; geometry construction belongs to a backend.
pub fn degree_change_knots(
    source_degree: usize,
    desired_degree: usize,
    deformable: bool,
    source_knots: &[Real],
) -> Result<Vec<Real>, GeometryError> {
    if desired_degree == 0 {
        return Err(GeometryError::InvalidDegree);
    }
    let (start, end) = clamped_basis_domain(source_degree, source_knots)?;
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

/// Remove one occurrence of an interior knot from a clamped basis. This
/// chooses knot structure only; a backend constructs the replacement controls.
pub fn knot_removal_knots(
    degree: usize,
    source_knots: &[Real],
    knot: Real,
) -> Result<Vec<Real>, GeometryError> {
    knot_group_removal_knots(degree, source_knots, knot, 1)
}

/// Remove a specified number of repetitions from one interior group in a
/// clamped basis. Backends supply replacement controls separately.
pub fn knot_group_removal_knots(
    degree: usize,
    source_knots: &[Real],
    knot: Real,
    removal_count: usize,
) -> Result<Vec<Real>, GeometryError> {
    let (start, end) = clamped_basis_domain(degree, source_knots)?;
    if !knot.is_finite() || knot <= start || knot >= end {
        return Err(GeometryError::NoRemovableKnot { parameter: knot });
    }
    let first = source_knots.partition_point(|k| *k < knot);
    let after = source_knots.partition_point(|k| *k <= knot);
    if after == first {
        return Err(GeometryError::NoRemovableKnot { parameter: knot });
    }
    if removal_count == 0 || after - first < removal_count {
        return Err(GeometryError::KnotRemovalSolveFailed);
    }
    let mut knots = source_knots.to_vec();
    knots.drain(after - removal_count..after);
    Ok(knots)
}

pub(crate) fn basis_interior_knot_groups(degree: usize, knots: &[Real]) -> Vec<(Real, usize)> {
    let start = knots[degree];
    let end = knots[knots.len() - degree - 1];
    let mut groups = Vec::new();
    let mut index = 0;
    while index < knots.len() {
        let knot = knots[index];
        let after = knots.partition_point(|candidate| *candidate <= knot);
        if knot > start && knot < end {
            groups.push((knot, after - index));
        }
        index = after;
    }
    groups
}

fn removable_knot_at_index(
    degree: usize,
    knots: &[Real],
    index: usize,
    parameter: Real,
) -> Result<Real, GeometryError> {
    let count = knots.len() - degree - 1;
    if index < degree + 1
        || index >= count
        || knots[index] <= knots[degree]
        || knots[index] >= knots[count]
    {
        return Err(GeometryError::NoRemovableKnot { parameter });
    }
    Ok(knots[index])
}

pub(crate) fn knot_removal_candidate_by_parameter(
    degree: usize,
    knots: &[Real],
    parameter: Real,
) -> Result<Real, GeometryError> {
    let count = knots
        .len()
        .checked_sub(degree)
        .and_then(|n| n.checked_sub(1))
        .ok_or(GeometryError::InvalidKnotVector {
            context: "invalid knot-removal source basis",
        })?;
    validate_direction(degree, count, knots)?;
    require_finite([parameter], "NURBS parameter")?;
    let domain_start = knots[degree];
    let domain_end = knots[count];
    if parameter < domain_start || parameter > domain_end {
        return Err(GeometryError::ParameterOutOfDomain {
            parameter,
            domain_start,
            domain_end,
        });
    }
    let active = &knots[degree..=count];
    let upper = active.partition_point(|k| *k <= parameter);
    let index = if upper == 0 {
        degree
    } else if upper == active.len() {
        count
    } else {
        let lower = degree + upper - 1;
        let next = degree + upper;
        let midpoint = stable_knot_mean(&[knots[lower], knots[next]])?;
        if parameter < midpoint { lower } else { next }
    };
    removable_knot_at_index(degree, knots, index, parameter)
}

fn clamped_basis_domain(
    source_degree: usize,
    source_knots: &[Real],
) -> Result<(Real, Real), GeometryError> {
    if source_degree == 0 {
        return Err(GeometryError::InvalidDegree);
    }
    let minimum = source_degree
        .checked_add(1)
        .and_then(|n| n.checked_mul(2))
        .ok_or(GeometryError::InvalidDegree)?;
    if source_knots.len() < minimum {
        return Err(GeometryError::InvalidKnotVector {
            context: "invalid clamped NURBS basis",
        });
    }
    validate_direction(
        source_degree,
        source_knots.len() - source_degree - 1,
        source_knots,
    )?;
    let start = source_knots[source_degree];
    let end = source_knots[source_knots.len() - source_degree - 1];
    if source_knots[..=source_degree].iter().any(|k| *k != start)
        || source_knots[source_knots.len() - source_degree - 1..]
            .iter()
            .any(|k| *k != end)
    {
        return Err(GeometryError::InvalidKnotVector {
            context: "NURBS basis must be clamped",
        });
    }
    Ok((start, end))
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

pub(crate) fn portable_periodic_interpolation(
    input: &crate::NurbsInterpolationSystem<'_>,
    _control_shift: usize,
) -> Result<Vec<Real>, GeometryError> {
    let count = input.parameters.len() - 1;
    let controls = count + input.degree;
    let rows = input.parameters[..count]
        .iter()
        .map(|parameter| {
            let basis = bspline_basis_values(input.knots, input.degree, controls, *parameter)?;
            let mut folded = vec![0.; count];
            for (index, value) in basis.into_iter().enumerate() {
                folded[index % count] += value;
            }
            Ok(folded)
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let matrix = Mat::from_fn(count, count, |row, column| rows[row][column]);
    let rhs = Mat::from_fn(count, input.columns, |row, column| {
        input.targets[row * input.columns + column]
    });
    let solution = matrix.full_piv_lu().solve(&rhs);
    Ok((0..count)
        .flat_map(|row| {
            let solution = &solution;
            (0..input.columns).map(move |column| solution[(row, column)])
        })
        .collect())
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

pub(crate) fn periodic_greville_parameters(
    degree: usize,
    control_count: usize,
    unique_count: usize,
    knots: &[Real],
) -> Result<(Vec<Real>, usize), GeometryError> {
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
    Ok((parameters, start_index))
}

pub(crate) fn stable_knot_mean(knots: &[Real]) -> Result<Real, GeometryError> {
    if let Some(first) = knots.first()
        && knots.iter().all(|knot| knot == first)
    {
        require_finite([*first], "periodic Greville abscissa")?;
        return Ok(*first);
    }
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
    bspline_basis_values_impl(knots, degree, control_count, parameter, false)
}

/// Basis of the nearest nonempty boundary span outside the active domain.
/// Unlike periodic wrapping, this is polynomial span continuation.
pub(crate) fn bspline_basis_values_extended(
    knots: &[Real],
    degree: usize,
    control_count: usize,
    parameter: Real,
) -> Result<Vec<Real>, GeometryError> {
    bspline_basis_values_impl(knots, degree, control_count, parameter, true)
}

fn bspline_basis_values_impl(
    knots: &[Real],
    degree: usize,
    control_count: usize,
    parameter: Real,
    extended: bool,
) -> Result<Vec<Real>, GeometryError> {
    debug_assert_eq!(knots.len(), control_count + degree + 1);
    debug_assert!(control_count > degree);
    let domain_start = knots[degree];
    let domain_end = knots[control_count];
    require_finite([parameter], "B-spline basis parameter")?;
    if !extended && (parameter < domain_start || parameter > domain_end) {
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
            let left_fraction = if extended {
                interval_fraction_unbounded(parameter, left_knot, right_knot)?
            } else {
                interval_fraction(parameter, left_knot, right_knot)?
            };
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

pub fn curve_points_coincident(left: Point3, right: Point3) -> bool {
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
    de_boor_impl::<DIMENSION, false, false>(knots, degree, span, parameter, &mut work)
}

/// Evaluate the polynomial represented by a selected span, including its
/// continuation outside that span. The caller must select a nonempty span.
pub(crate) fn de_boor_extended<const DIMENSION: usize>(
    knots: &[Real],
    degree: usize,
    span: usize,
    parameter: Real,
    mut work: Vec<[Real; DIMENSION]>,
) -> Result<[Real; DIMENSION], GeometryError> {
    de_boor_extended_in_place(knots, degree, span, parameter, &mut work)
}

/// Same recurrence as `de_boor_extended`, with caller-owned scratch storage.
/// The input is exactly the active span's degree-plus-one homogeneous controls.
pub(crate) fn de_boor_extended_in_place<const DIMENSION: usize>(
    knots: &[Real],
    degree: usize,
    span: usize,
    parameter: Real,
    work: &mut [[Real; DIMENSION]],
) -> Result<[Real; DIMENSION], GeometryError> {
    de_boor_impl::<DIMENSION, true, false>(knots, degree, span, parameter, work)
}

/// Polynomial continuation using affine differences outside each knot interval.
/// This preserves constant components even when `1 - alpha` loses its unit term.
/// An overflowing difference is an error; callers may recover using exact inputs.
pub(crate) fn de_boor_polynomial_extended<const DIMENSION: usize>(
    knots: &[Real],
    degree: usize,
    span: usize,
    parameter: Real,
    mut work: Vec<[Real; DIMENSION]>,
) -> Result<[Real; DIMENSION], GeometryError> {
    de_boor_impl::<DIMENSION, true, true>(knots, degree, span, parameter, &mut work)
}

fn de_boor_impl<const DIMENSION: usize, const EXTENDED: bool, const AFFINE_BLEND: bool>(
    knots: &[Real],
    degree: usize,
    span: usize,
    parameter: Real,
    work: &mut [[Real; DIMENSION]],
) -> Result<[Real; DIMENSION], GeometryError> {
    debug_assert_eq!(work.len(), degree + 1);
    for level in 1..=degree {
        for local_index in (level..=degree).rev() {
            let knot_index = span - degree + local_index;
            let left_knot = knots[knot_index];
            let right_knot = knots[knot_index + degree - level + 1];
            let alpha = if EXTENDED {
                interval_fraction_unbounded(parameter, left_knot, right_knot)?
            } else {
                interval_fraction(parameter, left_knot, right_knot)?
            };
            work[local_index] = if EXTENDED && !(0.0..=1.0).contains(&alpha) {
                let result = std::array::from_fn(|i| {
                    let a = work[local_index - 1][i];
                    let b = work[local_index][i];
                    if AFFINE_BLEND {
                        (b - a).mul_add(alpha, a)
                    } else {
                        a.mul_add(1.0 - alpha, b * alpha)
                    }
                });
                require_finite(result, "homogeneous NURBS continuation")?;
                result
            } else {
                blend_homogeneous(work[local_index - 1], work[local_index], alpha)?
            };
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

pub(crate) fn interval_fraction_unbounded(
    value: Real,
    interval_start: Real,
    interval_end: Real,
) -> Result<Real, GeometryError> {
    let denominator = interval_end - interval_start;
    let numerator = value - interval_start;
    let alpha = if denominator.is_finite() && denominator > 0.0 && numerator.is_finite() {
        numerator / denominator
    } else if (denominator.is_infinite() || numerator.is_infinite())
        && interval_start < interval_end
    {
        // Halving preserves the ratio when subtracting opposite, very large
        // finite coordinates would overflow, including off-interval controls
        // with a finite interval width.
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
    left.midpoint(right)
}

fn interpolate_parameter(start: Real, end: Real, fraction: Real) -> Real {
    // Exact endpoints and a final clamp prevent one-ulp excursions when
    // sampling a negative or very narrow knot interval.
    if fraction == 0.0 {
        return start;
    }
    if fraction == 1.0 {
        return end;
    }
    let parameter = if start.is_sign_negative() == end.is_sign_negative() {
        start + (end - start) * fraction
    } else {
        start * (1.0 - fraction) + end * fraction
    };
    parameter.clamp(start.min(end), start.max(end))
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
    left.midpoint(right)
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
            && (previous.end.second_parameter > previous.start.second_parameter)
                == (overlap.end.second_parameter > overlap.start.second_parameter)
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
mod tests;
