use std::ops::RangeInclusive;

use crate::integration::integrate_adaptive;
use crate::nurbs::{
    clamped_uniform_knots, control_polygon_range, curve_coordinates_coincident,
    curve_points_coincident, de_boor, knot_vector_is_periodic, project_homogeneous,
    stable_divided_difference, stable_knot_mean, uniform_knots_like, validate_direction,
};
use crate::{
    AffineTransform3, BoundingBox3, Frame3, GeometryError, MAX_CURVE_DIVISION_POINTS, MeshFace,
    NurbsCurve, Plane, Point3, Real, Tolerance, TriangleMesh, UnitVector3, Vector3, WeightedPoint3,
    require_finite, vector::product_three,
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

/// Parametric direction changed by [`NurbsSurface::try_make_uniform`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceKnotDirection {
    U,
    V,
    Both,
}

impl SurfaceKnotDirection {
    const fn includes_u(self) -> bool {
        matches!(self, Self::U | Self::Both)
    }

    const fn includes_v(self) -> bool {
        matches!(self, Self::V | Self::Both)
    }
}

fn homogeneous_controls_coincident(left: WeightedPoint3, right: WeightedPoint3) -> bool {
    if !curve_coordinates_coincident(left.weight(), right.weight()) {
        return false;
    }
    left.point()
        .to_array()
        .into_iter()
        .zip(right.point().to_array())
        .all(|(left_coordinate, right_coordinate)| {
            let left_homogeneous = left_coordinate * left.weight();
            let right_homogeneous = right_coordinate * right.weight();
            left_homogeneous.is_finite()
                && right_homogeneous.is_finite()
                && curve_coordinates_coincident(left_homogeneous, right_homogeneous)
        })
}

fn validate_surface_knot_insertion(
    parameter: Real,
    target_multiplicity: usize,
    degree: usize,
    domain: RangeInclusive<Real>,
) -> Result<(), GeometryError> {
    if target_multiplicity == 0 || target_multiplicity > degree {
        return Err(GeometryError::InvalidKnotMultiplicity {
            actual: target_multiplicity,
            maximum: degree,
        });
    }
    require_finite([parameter], "NURBS surface knot parameter")?;
    let domain_start = *domain.start();
    let domain_end = *domain.end();
    if parameter < domain_start || parameter > domain_end {
        return Err(GeometryError::ParameterOutOfDomain {
            parameter,
            domain_start,
            domain_end,
        });
    }
    if (parameter == domain_start || parameter == domain_end)
        && target_multiplicity != 1
        && target_multiplicity != degree
    {
        return Err(GeometryError::InvalidEndpointKnotMultiplicity {
            actual: target_multiplicity,
            degree,
        });
    }
    Ok(())
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
            if !control_point.weight().is_finite() || control_point.weight() == 0.0 {
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

    /// Constructs the exact fixed-orientation sweep of a NURBS profile along
    /// a NURBS path.
    ///
    /// This is the OpenNURBS sum-surface form used by Rhino's
    /// `ExtrudeCrvAlongCrv`: U preserves the profile, V preserves the path,
    /// the path start is subtracted as the base point, and each tensor weight
    /// is the product of its profile and path weights.
    pub fn try_extruded_curve_along_curve(
        profile: &crate::NurbsCurve,
        path: &crate::NurbsCurve,
    ) -> Result<Self, GeometryError> {
        let first_path_control = path.control_points()[0].point();
        if path
            .control_points()
            .iter()
            .all(|control| control.point() == first_path_control)
        {
            return Err(GeometryError::Degenerate {
                context: "curve extrusion path",
            });
        }
        let path_start = path.evaluate(*path.domain().start())?;
        let control_count_u = profile.control_points().len();
        let control_count_v = path.control_points().len();
        let control_count = control_count_u.checked_mul(control_count_v).ok_or(
            GeometryError::InvalidControlNet {
                context: "curve-along-curve control-point count overflowed usize",
            },
        )?;
        let mut controls = Vec::new();
        controls.try_reserve_exact(control_count).map_err(|_| {
            GeometryError::InvalidControlNet {
                context: "curve-along-curve control net exceeds addressable memory",
            }
        })?;
        for path_control in path.control_points() {
            let offset = path_start.vector_to(path_control.point())?;
            for profile_control in profile.control_points() {
                controls.push(WeightedPoint3::try_new(
                    profile_control.point().translated(offset)?,
                    profile_control.weight() * path_control.weight(),
                )?);
            }
        }
        Self::try_new_rational(
            profile.degree(),
            path.degree(),
            control_count_u,
            control_count_v,
            controls,
            profile.knots().to_vec(),
            path.knots().to_vec(),
        )
    }

    /// Constructs the exact ruled surface between a NURBS curve and one apex.
    ///
    /// Matching Rhino's `ExtrudeCrvToPoint` NURBS form, U runs from the source
    /// curve to the apex and V retains the source curve's degree, knots, and
    /// rational weights. The U domain is the distance from the curve's start
    /// point to the apex. Repeating the apex with each corresponding curve
    /// weight makes the collapsed U edge exact even for rational curves.
    pub fn try_extruded_curve_to_point(
        curve: &crate::NurbsCurve,
        apex: Point3,
    ) -> Result<Self, GeometryError> {
        let curve_start = curve.evaluate(*curve.domain().start())?;
        let apex_distance = curve_start.distance_to(apex)?;
        if apex_distance == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "curve-to-point extrusion path",
            });
        }

        let control_count_v = curve.control_points().len();
        let control_count =
            control_count_v
                .checked_mul(2)
                .ok_or(GeometryError::InvalidControlNet {
                    context: "curve-to-point control-point count overflowed usize",
                })?;
        let mut controls = Vec::new();
        controls.try_reserve_exact(control_count).map_err(|_| {
            GeometryError::InvalidControlNet {
                context: "curve-to-point control net exceeds addressable memory",
            }
        })?;
        for control in curve.control_points() {
            controls.push(*control);
            controls.push(WeightedPoint3::try_new(apex, control.weight())?);
        }
        Self::try_new_rational(
            1,
            curve.degree(),
            2,
            control_count_v,
            controls,
            vec![0.0, 0.0, apex_distance, apex_distance],
            curve.knots().to_vec(),
        )
    }

    /// Constructs the exact rational quadratic NURBS form of a sphere.
    ///
    /// The 9-by-5 control net, fully multiple quadrant knots, longitude
    /// domain `[0, 2π]`, and latitude domain `[-π/2, π/2]` match
    /// `ON_Sphere::GetNurbForm`. U follows the frame's xy plane and V runs
    /// from the negative to the positive frame z-axis.
    pub fn try_sphere(frame: Frame3, radius: Real) -> Result<Self, GeometryError> {
        require_finite([radius], "sphere radius")?;
        if radius <= 0.0 {
            return Err(GeometryError::Degenerate { context: "sphere" });
        }

        let longitude_coordinates: [[Real; 2]; 9] = [
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [-1.0, 1.0],
            [-1.0, 0.0],
            [-1.0, -1.0],
            [0.0, -1.0],
            [1.0, -1.0],
            [1.0, 0.0],
        ];
        let diagonal_weight = std::f64::consts::FRAC_1_SQRT_2;
        let longitude_weights: [Real; 9] = [
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
        ];
        let latitude_coordinates: [[Real; 2]; 5] =
            [[0.0, -1.0], [1.0, -1.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let latitude_weights: [Real; 5] = [1.0, diagonal_weight, 1.0, diagonal_weight, 1.0];
        let origin = frame.origin().to_array();
        let x_axis = frame.x_axis().as_vector().to_array();
        let y_axis = frame.y_axis().as_vector().to_array();
        let z_axis = frame.z_axis().as_vector().to_array();
        let mut controls = Vec::with_capacity(45);
        for ([radial, height], latitude_weight) in
            latitude_coordinates.into_iter().zip(latitude_weights)
        {
            let radial_scale = radius * radial;
            let height_scale = radius * height;
            for ([x, y], longitude_weight) in
                longitude_coordinates.into_iter().zip(longitude_weights)
            {
                let point = Point3::try_from(std::array::from_fn(|coordinate| {
                    let radial_coordinate = x.mul_add(x_axis[coordinate], y * y_axis[coordinate]);
                    radial_scale.mul_add(
                        radial_coordinate,
                        height_scale.mul_add(z_axis[coordinate], origin[coordinate]),
                    )
                }))?;
                controls.push(WeightedPoint3::try_new(
                    point,
                    longitude_weight * latitude_weight,
                )?);
            }
        }

        let half_pi = std::f64::consts::FRAC_PI_2;
        let pi = std::f64::consts::PI;
        let three_half_pi = 3.0 * half_pi;
        let tau = std::f64::consts::TAU;
        Self::try_new_rational(
            2,
            2,
            9,
            5,
            controls,
            vec![
                0.0,
                0.0,
                0.0,
                half_pi,
                half_pi,
                pi,
                pi,
                three_half_pi,
                three_half_pi,
                tau,
                tau,
                tau,
            ],
            vec![
                -half_pi, -half_pi, -half_pi, 0.0, 0.0, half_pi, half_pi, half_pi,
            ],
        )
    }

    /// Constructs an exact rational quadratic NURBS ellipsoid.
    ///
    /// An ellipsoid is the affine image of the exact 9-by-5 sphere surface:
    /// its control weights, fully multiple quadrant knots, longitude domain
    /// `[0, 2π]`, and latitude domain `[-π/2, π/2]` are unchanged. The three
    /// positive semi-axis radii scale the supplied frame's x, y, and z axes.
    pub fn try_ellipsoid(frame: Frame3, radii: [Real; 3]) -> Result<Self, GeometryError> {
        require_finite(radii, "ellipsoid radii")?;
        if radii.into_iter().any(|radius| radius <= 0.0) {
            return Err(GeometryError::Degenerate {
                context: "ellipsoid",
            });
        }
        let transform = AffineTransform3::try_frame_mapping(frame, frame, radii)?;
        Self::try_sphere(frame, 1.0)?.transformed(transform)
    }

    /// Constructs an exact rational polar disk in the supplied frame.
    ///
    /// U is a four-span quadratic circle on `[0, 2π]`; V is the radial
    /// distance on `[0, radius]`. The V-start boundary collapses to the center,
    /// and the natural surface normal points opposite the frame's Z axis.
    pub fn try_disk(frame: Frame3, radius: Real) -> Result<Self, GeometryError> {
        require_finite([radius], "disk radius")?;
        if radius <= 0.0 {
            return Err(GeometryError::Degenerate { context: "disk" });
        }
        let circle_coordinates: [[Real; 2]; 9] = [
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [-1.0, 1.0],
            [-1.0, 0.0],
            [-1.0, -1.0],
            [0.0, -1.0],
            [1.0, -1.0],
            [1.0, 0.0],
        ];
        let diagonal_weight = std::f64::consts::FRAC_1_SQRT_2;
        let circle_weights: [Real; 9] = [
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
        ];
        let origin = frame.origin().to_array();
        let x_axis = frame.x_axis().as_vector().to_array();
        let y_axis = frame.y_axis().as_vector().to_array();
        let mut controls = circle_weights
            .into_iter()
            .map(|weight| WeightedPoint3::try_new(frame.origin(), weight))
            .collect::<Result<Vec<_>, _>>()?;
        for ([x, y], weight) in circle_coordinates.into_iter().zip(circle_weights) {
            let point = Point3::try_from(std::array::from_fn(|coordinate| {
                let radial_coordinate = x.mul_add(x_axis[coordinate], y * y_axis[coordinate]);
                radius.mul_add(radial_coordinate, origin[coordinate])
            }))?;
            controls.push(WeightedPoint3::try_new(point, weight)?);
        }
        let half_pi = std::f64::consts::FRAC_PI_2;
        let pi = std::f64::consts::PI;
        let tau = std::f64::consts::TAU;
        Self::try_new_rational(
            2,
            1,
            9,
            2,
            controls,
            vec![
                0.0,
                0.0,
                0.0,
                half_pi,
                half_pi,
                pi,
                pi,
                3.0 * half_pi,
                3.0 * half_pi,
                tau,
                tau,
                tau,
            ],
            vec![0.0, 0.0, radius, radius],
        )
    }

    /// Constructs the exact open NURBS wall of a right circular cylinder.
    ///
    /// U is Rhino/OpenNURBS' four-span rational quadratic circle on
    /// `[0, 2π]`. V is linear over the increasing signed height interval.
    /// The supplied frame origin is height zero; reversed endpoints therefore
    /// preserve the same surface and parameterization.
    pub fn try_cylinder(
        frame: Frame3,
        radius: Real,
        start_height: Real,
        end_height: Real,
    ) -> Result<Self, GeometryError> {
        require_finite([radius, start_height, end_height], "cylinder dimensions")?;
        if radius <= 0.0 || start_height == end_height {
            return Err(GeometryError::Degenerate {
                context: "cylinder",
            });
        }

        let circle_coordinates: [[Real; 2]; 9] = [
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [-1.0, 1.0],
            [-1.0, 0.0],
            [-1.0, -1.0],
            [0.0, -1.0],
            [1.0, -1.0],
            [1.0, 0.0],
        ];
        let diagonal_weight = std::f64::consts::FRAC_1_SQRT_2;
        let circle_weights: [Real; 9] = [
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
        ];
        let origin = frame.origin().to_array();
        let x_axis = frame.x_axis().as_vector().to_array();
        let y_axis = frame.y_axis().as_vector().to_array();
        let z_axis = frame.z_axis().as_vector().to_array();
        let [height_start, height_end] = if start_height < end_height {
            [start_height, end_height]
        } else {
            [end_height, start_height]
        };
        let mut controls = Vec::with_capacity(18);
        for height in [height_start, height_end] {
            for ([x, y], weight) in circle_coordinates.into_iter().zip(circle_weights) {
                let point = Point3::try_from(std::array::from_fn(|coordinate| {
                    let radial_coordinate = x.mul_add(x_axis[coordinate], y * y_axis[coordinate]);
                    radius.mul_add(
                        radial_coordinate,
                        height.mul_add(z_axis[coordinate], origin[coordinate]),
                    )
                }))?;
                controls.push(WeightedPoint3::try_new(point, weight)?);
            }
        }

        let half_pi = std::f64::consts::FRAC_PI_2;
        let pi = std::f64::consts::PI;
        let tau = std::f64::consts::TAU;
        Self::try_new_rational(
            2,
            1,
            9,
            2,
            controls,
            vec![
                0.0,
                0.0,
                0.0,
                half_pi,
                half_pi,
                pi,
                pi,
                3.0 * half_pi,
                3.0 * half_pi,
                tau,
                tau,
                tau,
            ],
            vec![height_start, height_start, height_end, height_end],
        )
    }

    /// Constructs the exact open NURBS wall of a right circular truncated cone.
    ///
    /// The frame origin is the base center and frame Z points toward the end
    /// circle. U is the four-span rational quadratic circle on `[0, 2π]` and
    /// V is linear over the generatrix's physical slant length, matching the
    /// NURBS form created by Rhino's `TruncatedCone` command.
    pub fn try_truncated_cone(
        frame: Frame3,
        radii: [Real; 2],
        height: Real,
    ) -> Result<Self, GeometryError> {
        require_finite(
            radii.into_iter().chain(std::iter::once(height)),
            "truncated-cone dimensions",
        )?;
        if radii.into_iter().any(|radius| radius <= 0.0) || height <= 0.0 {
            return Err(GeometryError::Degenerate {
                context: "truncated cone",
            });
        }
        let slant_length = height.hypot(radii[1] - radii[0]);
        require_finite([slant_length], "truncated-cone slant length")?;

        let circle_coordinates: [[Real; 2]; 9] = [
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [-1.0, 1.0],
            [-1.0, 0.0],
            [-1.0, -1.0],
            [0.0, -1.0],
            [1.0, -1.0],
            [1.0, 0.0],
        ];
        let diagonal_weight = std::f64::consts::FRAC_1_SQRT_2;
        let circle_weights: [Real; 9] = [
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
        ];
        let origin = frame.origin().to_array();
        let x_axis = frame.x_axis().as_vector().to_array();
        let y_axis = frame.y_axis().as_vector().to_array();
        let z_axis = frame.z_axis().as_vector().to_array();
        let mut controls = Vec::with_capacity(18);
        for (radius, ring_height) in [(radii[0], 0.0), (radii[1], height)] {
            for ([x, y], weight) in circle_coordinates.into_iter().zip(circle_weights) {
                let point = Point3::try_from(std::array::from_fn(|coordinate| {
                    let radial_coordinate = x.mul_add(x_axis[coordinate], y * y_axis[coordinate]);
                    radius.mul_add(
                        radial_coordinate,
                        ring_height.mul_add(z_axis[coordinate], origin[coordinate]),
                    )
                }))?;
                controls.push(WeightedPoint3::try_new(point, weight)?);
            }
        }

        let half_pi = std::f64::consts::FRAC_PI_2;
        let pi = std::f64::consts::PI;
        let tau = std::f64::consts::TAU;
        Self::try_new_rational(
            2,
            1,
            9,
            2,
            controls,
            vec![
                0.0,
                0.0,
                0.0,
                half_pi,
                half_pi,
                pi,
                pi,
                3.0 * half_pi,
                3.0 * half_pi,
                tau,
                tau,
                tau,
            ],
            vec![0.0, 0.0, slant_length, slant_length],
        )
    }

    /// Constructs the exact open NURBS wall of a right circular cone.
    ///
    /// This follows `ON_Cone::GetNurbForm`: the frame origin is the apex,
    /// `height_to_base` is the signed base offset on frame Z, U is the
    /// four-span rational quadratic circle on `[0, 2π]`, and V is an
    /// increasing linear height interval with the apex controls repeated at
    /// their corresponding circular weights.
    pub fn try_cone(
        apex_frame: Frame3,
        radius: Real,
        height_to_base: Real,
    ) -> Result<Self, GeometryError> {
        require_finite([radius, height_to_base], "cone dimensions")?;
        if radius <= 0.0 || height_to_base == 0.0 {
            return Err(GeometryError::Degenerate { context: "cone" });
        }

        let circle_coordinates: [[Real; 2]; 9] = [
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [-1.0, 1.0],
            [-1.0, 0.0],
            [-1.0, -1.0],
            [0.0, -1.0],
            [1.0, -1.0],
            [1.0, 0.0],
        ];
        let diagonal_weight = std::f64::consts::FRAC_1_SQRT_2;
        let circle_weights: [Real; 9] = [
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
        ];
        let origin = apex_frame.origin().to_array();
        let x_axis = apex_frame.x_axis().as_vector().to_array();
        let y_axis = apex_frame.y_axis().as_vector().to_array();
        let z_axis = apex_frame.z_axis().as_vector().to_array();
        let mut base_controls = Vec::with_capacity(9);
        for ([x, y], weight) in circle_coordinates.into_iter().zip(circle_weights) {
            let point = Point3::try_from(std::array::from_fn(|coordinate| {
                let radial_coordinate = x.mul_add(x_axis[coordinate], y * y_axis[coordinate]);
                radius.mul_add(
                    radial_coordinate,
                    height_to_base.mul_add(z_axis[coordinate], origin[coordinate]),
                )
            }))?;
            base_controls.push(WeightedPoint3::try_new(point, weight)?);
        }
        let apex_controls = circle_weights
            .into_iter()
            .map(|weight| WeightedPoint3::try_new(apex_frame.origin(), weight))
            .collect::<Result<Vec<_>, _>>()?;
        let (height_start, height_end, controls) = if height_to_base < 0.0 {
            let mut controls = base_controls;
            controls.extend(apex_controls);
            (height_to_base, 0.0, controls)
        } else {
            let mut controls = apex_controls;
            controls.extend(base_controls);
            (0.0, height_to_base, controls)
        };

        let half_pi = std::f64::consts::FRAC_PI_2;
        let pi = std::f64::consts::PI;
        let tau = std::f64::consts::TAU;
        Self::try_new_rational(
            2,
            1,
            9,
            2,
            controls,
            vec![
                0.0,
                0.0,
                0.0,
                half_pi,
                half_pi,
                pi,
                pi,
                3.0 * half_pi,
                3.0 * half_pi,
                tau,
                tau,
                tau,
            ],
            vec![height_start, height_start, height_end, height_end],
        )
    }

    /// Constructs Rhino's exact open NURBS paraboloid surface.
    ///
    /// The frame origin is the vertex, frame Z is the opening direction, and
    /// frame X reaches the surface seam at the circular rim. U is the
    /// four-span rational quadratic circle on `[0, 2π]`. V is a single
    /// quadratic Bezier parabola whose domain is its exact meridian arc
    /// length, matching the NURBS form produced by Rhino's `Paraboloid`
    /// command.
    pub fn try_paraboloid(
        vertex_frame: Frame3,
        radius: Real,
        height: Real,
    ) -> Result<Self, GeometryError> {
        require_finite([radius, height], "paraboloid dimensions")?;
        if radius <= 0.0 || height <= 0.0 {
            return Err(GeometryError::Degenerate {
                context: "paraboloid",
            });
        }

        let circle_coordinates: [[Real; 2]; 9] = [
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [-1.0, 1.0],
            [-1.0, 0.0],
            [-1.0, -1.0],
            [0.0, -1.0],
            [1.0, -1.0],
            [1.0, 0.0],
        ];
        let diagonal_weight = std::f64::consts::FRAC_1_SQRT_2;
        let circle_weights: [Real; 9] = [
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
        ];
        let origin = vertex_frame.origin().to_array();
        let x_axis = vertex_frame.x_axis().as_vector().to_array();
        let y_axis = vertex_frame.y_axis().as_vector().to_array();
        let z_axis = vertex_frame.z_axis().as_vector().to_array();
        let mut controls = Vec::with_capacity(27);
        for (ring_radius, ring_height) in [(0.0, 0.0), (0.5 * radius, 0.0), (radius, height)] {
            for ([x, y], weight) in circle_coordinates.into_iter().zip(circle_weights) {
                let point = Point3::try_from(std::array::from_fn(|coordinate| {
                    let radial_coordinate = x.mul_add(x_axis[coordinate], y * y_axis[coordinate]);
                    ring_radius.mul_add(
                        radial_coordinate,
                        ring_height.mul_add(z_axis[coordinate], origin[coordinate]),
                    )
                }))?;
                controls.push(WeightedPoint3::try_new(point, weight)?);
            }
        }

        // Integral from r=0 to r=radius of
        // sqrt(1 + (2*height*r/radius^2)^2) dr. This arrangement remains
        // well behaved for both shallow and steep finite paraboloids.
        let half_radius = 0.5 * radius;
        let slope = height / half_radius;
        let shallow_term = if slope == 0.0 {
            half_radius
        } else if slope.is_infinite() {
            0.0
        } else {
            half_radius * slope.asinh() / slope
        };
        let meridian_length = height.hypot(half_radius) + shallow_term;
        require_finite([meridian_length], "paraboloid meridian length")?;

        let half_pi = std::f64::consts::FRAC_PI_2;
        let pi = std::f64::consts::PI;
        let tau = std::f64::consts::TAU;
        Self::try_new_rational(
            2,
            2,
            9,
            3,
            controls,
            vec![
                0.0,
                0.0,
                0.0,
                half_pi,
                half_pi,
                pi,
                pi,
                3.0 * half_pi,
                3.0 * half_pi,
                tau,
                tau,
                tau,
            ],
            vec![
                0.0,
                0.0,
                0.0,
                meridian_length,
                meridian_length,
                meridian_length,
            ],
        )
    }

    /// Constructs the exact rational quadratic NURBS form of a ring torus.
    ///
    /// Both directions have four quadratic spans and nine controls. Matching
    /// `ON_Torus::GetNurbForm`, U follows the major circle with an arc-length
    /// domain of `2π * major_radius`, while V follows the minor circle with an
    /// arc-length domain of `2π * minor_radius`. Tensor weights are the exact
    /// products of the two circular weights.
    pub fn try_torus(
        frame: Frame3,
        major_radius: Real,
        minor_radius: Real,
    ) -> Result<Self, GeometryError> {
        require_finite([major_radius, minor_radius], "torus radii")?;
        if minor_radius <= 0.0 || major_radius <= minor_radius {
            return Err(GeometryError::Degenerate { context: "torus" });
        }
        let domain_u = std::f64::consts::TAU * major_radius;
        let domain_v = std::f64::consts::TAU * minor_radius;
        require_finite([domain_u, domain_v], "torus parameter domains")?;

        let circle_coordinates: [[Real; 2]; 9] = [
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [-1.0, 1.0],
            [-1.0, 0.0],
            [-1.0, -1.0],
            [0.0, -1.0],
            [1.0, -1.0],
            [1.0, 0.0],
        ];
        let diagonal_weight = std::f64::consts::FRAC_1_SQRT_2;
        let circle_weights: [Real; 9] = [
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
            diagonal_weight,
            1.0,
        ];
        let origin = frame.origin().to_array();
        let x_axis = frame.x_axis().as_vector().to_array();
        let y_axis = frame.y_axis().as_vector().to_array();
        let z_axis = frame.z_axis().as_vector().to_array();
        let mut controls = Vec::with_capacity(81);
        for ([minor_radial, minor_height], minor_weight) in
            circle_coordinates.into_iter().zip(circle_weights)
        {
            let radial = minor_radius.mul_add(minor_radial, major_radius);
            let height = minor_radius * minor_height;
            for ([major_x, major_y], major_weight) in
                circle_coordinates.into_iter().zip(circle_weights)
            {
                let point = Point3::try_from(std::array::from_fn(|coordinate| {
                    let radial_coordinate =
                        major_x.mul_add(x_axis[coordinate], major_y * y_axis[coordinate]);
                    radial.mul_add(
                        radial_coordinate,
                        height.mul_add(z_axis[coordinate], origin[coordinate]),
                    )
                }))?;
                controls.push(WeightedPoint3::try_new(point, major_weight * minor_weight)?);
            }
        }

        let circle_knots = |domain: Real| {
            let quarter = domain * 0.25;
            let half = domain * 0.5;
            let three_quarters = domain * 0.75;
            vec![
                0.0,
                0.0,
                0.0,
                quarter,
                quarter,
                half,
                half,
                three_quarters,
                three_quarters,
                domain,
                domain,
                domain,
            ]
        };
        Self::try_new_rational(
            2,
            2,
            9,
            9,
            controls,
            circle_knots(domain_u),
            circle_knots(domain_v),
        )
    }

    /// Constructs an exact rational surface by revolving a NURBS profile.
    ///
    /// U is the quadratic rational revolution direction and V preserves the
    /// profile's degree and complete knot vector. The U domain follows Rhino's
    /// exact-revolve convention: sweep radians multiplied by the profile's
    /// OpenNURBS 65-sample maximum-radius estimate. Quadrant knots are fully
    /// multiple and every surface weight is the product of its angular and
    /// profile-curve weights.
    pub fn try_revolved_curve(
        curve: &crate::NurbsCurve,
        axis_origin: Point3,
        axis_direction: UnitVector3,
        start_angle_radians: Real,
        sweep_angle_radians: Real,
    ) -> Result<Self, GeometryError> {
        require_finite([start_angle_radians], "revolution start angle")?;
        if !sweep_angle_radians.is_finite()
            || sweep_angle_radians == 0.0
            || sweep_angle_radians.abs() > std::f64::consts::TAU
        {
            return Err(GeometryError::InvalidRevolutionSweep);
        }

        let axis_vector = axis_direction.as_vector();
        let axis_center = |point: Point3| -> Result<Point3, GeometryError> {
            let offset = axis_origin.vector_to(point)?;
            axis_origin.translated(axis_vector.scaled(offset.dot(axis_vector)?)?)
        };
        let mut radius_estimate: Real = 0.0;
        for index in 0..=64 {
            let point = curve.evaluate(curve.parameter_at(index as Real / 64.0)?)?;
            radius_estimate = radius_estimate.max(point.distance_to(axis_center(point)?)?);
        }
        if radius_estimate == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "revolution profile radius",
            });
        }

        let sweep_magnitude = sweep_angle_radians.abs();
        let quadrant_limit = (0.5 + f64::EPSILON.sqrt()) * std::f64::consts::PI;
        let span_count: usize = if sweep_magnitude <= quadrant_limit {
            1
        } else if sweep_magnitude <= 2.0 * quadrant_limit {
            2
        } else {
            4
        };
        let segment_angle = sweep_angle_radians / span_count as Real;
        let middle_weight = (0.5 * segment_angle).cos();
        let domain_length = radius_estimate * sweep_magnitude;
        require_finite([domain_length], "revolution parameter domain")?;

        let control_count_u = span_count
            .checked_mul(2)
            .and_then(|count| count.checked_add(1))
            .ok_or(GeometryError::InvalidControlNet {
                context: "revolution control-point count overflowed usize",
            })?;
        let control_count_v = curve.control_points().len();
        let control_count = control_count_u.checked_mul(control_count_v).ok_or(
            GeometryError::InvalidControlNet {
                context: "revolution control-net size overflowed usize",
            },
        )?;
        let mut controls = Vec::new();
        controls.try_reserve_exact(control_count).map_err(|_| {
            GeometryError::InvalidControlNet {
                context: "revolution control net exceeds addressable memory",
            }
        })?;

        let normalized_start = start_angle_radians.rem_euclid(std::f64::consts::TAU);
        let full_turn = sweep_magnitude == std::f64::consts::TAU;
        for profile_control in curve.control_points() {
            let point = profile_control.point();
            let center = axis_center(point)?;
            let first =
                AffineTransform3::try_rotation(axis_origin, axis_direction, normalized_start)?
                    .transform_point(point)?;
            controls.push(WeightedPoint3::try_new(first, profile_control.weight())?);
            for span in 0..span_count {
                let middle_angle = normalized_start + (span as Real + 0.5) * segment_angle;
                let rotated_middle =
                    AffineTransform3::try_rotation(axis_origin, axis_direction, middle_angle)?
                        .transform_point(point)?;
                let middle = center.translated(
                    center
                        .vector_to(rotated_middle)?
                        .scaled(middle_weight.recip())?,
                )?;
                controls.push(WeightedPoint3::try_new(
                    middle,
                    profile_control.weight() * middle_weight,
                )?);

                let endpoint = if full_turn && span + 1 == span_count {
                    first
                } else {
                    AffineTransform3::try_rotation(
                        axis_origin,
                        axis_direction,
                        normalized_start + (span + 1) as Real * segment_angle,
                    )?
                    .transform_point(point)?
                };
                controls.push(WeightedPoint3::try_new(endpoint, profile_control.weight())?);
            }
        }

        let mut knots_u = Vec::with_capacity(control_count_u + 3);
        knots_u.extend([0.0; 3]);
        for boundary in 1..span_count {
            let knot = domain_length * boundary as Real / span_count as Real;
            knots_u.extend([knot; 2]);
        }
        knots_u.extend([domain_length; 3]);
        Self::try_new_rational(
            2,
            curve.degree(),
            control_count_u,
            control_count_v,
            controls,
            knots_u,
            curve.knots().to_vec(),
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

    /// Replaces the selected knot direction(s) with Rhino-compatible unit
    /// spacing without changing the degree, control net, or rational weights.
    ///
    /// Each direction retains its start and end clamping independently, and
    /// periodic control-net topology remains periodic. The surface shape and
    /// active parameter domains can change.
    pub fn try_make_uniform(&self, direction: SurfaceKnotDirection) -> Result<Self, GeometryError> {
        let knots_u = if direction.includes_u() {
            uniform_knots_like(self.degree_u, self.control_point_count_u, &self.knots_u)?
        } else {
            self.knots_u.clone()
        };
        let knots_v = if direction.includes_v() {
            uniform_knots_like(self.degree_v, self.control_point_count_v, &self.knots_v)?
        } else {
            self.knots_v.clone()
        };
        Self::try_new_rational(
            self.degree_u,
            self.degree_v,
            self.control_point_count_u,
            self.control_point_count_v,
            self.control_points.clone(),
            knots_u,
            knots_v,
        )
    }

    /// Changes the polynomial degree in each parameter direction using the
    /// same knot and Greville-collocation rules as [`crate::NurbsCurve`].
    ///
    /// Directions whose requested degree is unchanged retain their existing
    /// representation. Raising with `deformable = false` is exact; lowering,
    /// and either direction with `deformable = true`, uses simple interior
    /// knots and interpolates the original homogeneous surface.
    pub fn try_change_degree(
        &self,
        desired_degree_u: usize,
        desired_degree_v: usize,
        deformable: bool,
    ) -> Result<Self, GeometryError> {
        if desired_degree_u == 0 || desired_degree_v == 0 {
            return Err(GeometryError::InvalidDegree);
        }
        let mut result = self.clone();
        if desired_degree_u != result.degree_u {
            result = result.map_u_control_curves(|curve| {
                curve.try_change_degree(desired_degree_u, deformable)
            })?;
        }
        if desired_degree_v != result.degree_v {
            result = result.map_v_control_curves(|curve| {
                curve.try_change_degree(desired_degree_v, deformable)
            })?;
        }
        Ok(result)
    }

    /// Converts selected periodic directions to equivalent clamped,
    /// non-periodic form without changing the active parameterization or
    /// surface locus. Directions that are already non-periodic are unchanged.
    pub fn try_make_non_periodic(
        &self,
        direction: SurfaceKnotDirection,
    ) -> Result<Self, GeometryError> {
        let clamp_u = direction.includes_u() && self.is_periodic_u();
        let clamp_v = direction.includes_v() && self.is_periodic_v();
        let mut result = self.clone();
        if clamp_u {
            result = result.clamped_in_u()?;
        }
        if clamp_v {
            result = result.clamped_in_v()?;
        }
        Ok(result)
    }

    /// Converts selected closed degree-two-or-higher directions to
    /// Rhino-compatible periodic form.
    ///
    /// With `smooth = false`, the existing homogeneous control net is changed
    /// minimally at each seam. With `smooth = true`, the active knot breaks are
    /// retained and the seam controls are interpolated in homogeneous space.
    /// Directions that are already periodic are unchanged.
    pub fn try_make_periodic(
        &self,
        direction: SurfaceKnotDirection,
        smooth: bool,
    ) -> Result<Self, GeometryError> {
        let make_u = direction.includes_u() && !self.is_periodic_u();
        let make_v = direction.includes_v() && !self.is_periodic_v();
        if make_u && !self.is_closed_u()? {
            return Err(GeometryError::PeriodicSurfaceDirectionMustBeClosed { direction: "U" });
        }
        if make_v && !self.is_closed_v()? {
            return Err(GeometryError::PeriodicSurfaceDirectionMustBeClosed { direction: "V" });
        }

        let mut result = self.clone();
        if make_u {
            result = result
                .map_u_control_curves(|curve| curve.try_make_periodic_assuming_closed(smooth))?;
        }
        if make_v {
            result = result
                .map_v_control_curves(|curve| curve.try_make_periodic_assuming_closed(smooth))?;
        }
        Ok(result)
    }

    fn clamped_in_u(&self) -> Result<Self, GeometryError> {
        let mut controls = Vec::new();
        let mut clamped_u_count = None;
        let mut clamped_knots_u = None;
        for v in 0..self.control_point_count_v {
            let start = v * self.control_point_count_u;
            let row = NurbsCurve::try_new_rational(
                self.degree_u,
                self.control_points[start..start + self.control_point_count_u].to_vec(),
                self.knots_u.clone(),
            )?
            .clamped_to_active_domain()?;
            if clamped_u_count.is_none() {
                let count = row.control_points().len();
                let total = count.checked_mul(self.control_point_count_v).ok_or(
                    GeometryError::InvalidControlNet {
                        context: "clamped U control-net size overflowed usize",
                    },
                )?;
                controls.try_reserve_exact(total).map_err(|_| {
                    GeometryError::InvalidControlNet {
                        context: "clamped U control net exceeds addressable memory",
                    }
                })?;
                clamped_u_count = Some(count);
                clamped_knots_u = Some(row.knots().to_vec());
            }
            debug_assert_eq!(clamped_u_count, Some(row.control_points().len()));
            debug_assert_eq!(clamped_knots_u.as_deref(), Some(row.knots()));
            controls.extend_from_slice(row.control_points());
        }
        Self::try_new_rational(
            self.degree_u,
            self.degree_v,
            clamped_u_count.expect("a valid surface has at least one V row"),
            self.control_point_count_v,
            controls,
            clamped_knots_u.expect("a valid surface has a U knot vector"),
            self.knots_v.clone(),
        )
    }

    fn clamped_in_v(&self) -> Result<Self, GeometryError> {
        let mut columns = Vec::new();
        columns
            .try_reserve_exact(self.control_point_count_u)
            .map_err(|_| GeometryError::InvalidControlNet {
                context: "clamped V column list exceeds addressable memory",
            })?;
        let mut clamped_v_count = None;
        let mut clamped_knots_v = None;
        for u in 0..self.control_point_count_u {
            let column = (0..self.control_point_count_v)
                .map(|v| self.control_points[self.control_index(u, v)])
                .collect::<Vec<_>>();
            let column = NurbsCurve::try_new_rational(self.degree_v, column, self.knots_v.clone())?
                .clamped_to_active_domain()?;
            if clamped_v_count.is_none() {
                clamped_v_count = Some(column.control_points().len());
                clamped_knots_v = Some(column.knots().to_vec());
            }
            debug_assert_eq!(clamped_v_count, Some(column.control_points().len()));
            debug_assert_eq!(clamped_knots_v.as_deref(), Some(column.knots()));
            columns.push(column);
        }

        let clamped_v_count = clamped_v_count.expect("a valid surface has at least one U column");
        let total = self
            .control_point_count_u
            .checked_mul(clamped_v_count)
            .ok_or(GeometryError::InvalidControlNet {
                context: "clamped V control-net size overflowed usize",
            })?;
        let mut controls = Vec::new();
        controls
            .try_reserve_exact(total)
            .map_err(|_| GeometryError::InvalidControlNet {
                context: "clamped V control net exceeds addressable memory",
            })?;
        for v in 0..clamped_v_count {
            controls.extend(columns.iter().map(|column| column.control_points()[v]));
        }
        Self::try_new_rational(
            self.degree_u,
            self.degree_v,
            self.control_point_count_u,
            clamped_v_count,
            controls,
            self.knots_u.clone(),
            clamped_knots_v.expect("a valid surface has a V knot vector"),
        )
    }

    /// Inserts a U-direction knot to a target multiplicity without changing
    /// the surface locus or parameterization.
    ///
    /// Every fixed-V control row is refined by the exact curve algorithm, so
    /// rational weights and eligible periodic U topology are retained. Target
    /// multiplicity follows OpenNURBS and ranges from one through the U degree;
    /// an endpoint accepts only one (a no-op) or the degree (full clamping).
    pub fn try_insert_knot_u(
        &self,
        parameter: Real,
        target_multiplicity: usize,
    ) -> Result<Self, GeometryError> {
        validate_surface_knot_insertion(
            parameter,
            target_multiplicity,
            self.degree_u,
            self.domain_u(),
        )?;
        let restore_periodic_topology = self.insertion_curve_is_periodic_u();
        let mut controls = Vec::new();
        let mut refined_u_count = None;
        let mut refined_knots_u = None;
        for v in 0..self.control_point_count_v {
            let start = v * self.control_point_count_u;
            let row = NurbsCurve::try_new_rational(
                self.degree_u,
                self.control_points[start..start + self.control_point_count_u].to_vec(),
                self.knots_u.clone(),
            )?
            .try_insert_knot_with_periodic_topology(
                parameter,
                target_multiplicity,
                restore_periodic_topology,
            )?;
            if refined_u_count.is_none() {
                let count = row.control_points().len();
                let total = count.checked_mul(self.control_point_count_v).ok_or(
                    GeometryError::InvalidControlNet {
                        context: "inserted U control-net size overflowed usize",
                    },
                )?;
                controls.try_reserve_exact(total).map_err(|_| {
                    GeometryError::InvalidControlNet {
                        context: "inserted U control net exceeds addressable memory",
                    }
                })?;
                refined_u_count = Some(count);
                refined_knots_u = Some(row.knots().to_vec());
            }
            debug_assert_eq!(refined_u_count, Some(row.control_points().len()));
            debug_assert_eq!(refined_knots_u.as_deref(), Some(row.knots()));
            controls.extend_from_slice(row.control_points());
        }

        Self::try_new_rational(
            self.degree_u,
            self.degree_v,
            refined_u_count.expect("a valid surface has at least one V row"),
            self.control_point_count_v,
            controls,
            refined_knots_u.expect("a valid surface has a U knot vector"),
            self.knots_v.clone(),
        )
    }

    /// Inserts a V-direction knot to a target multiplicity without changing
    /// the surface locus or parameterization.
    ///
    /// Every fixed-U control column is refined by the exact curve algorithm,
    /// then transposed back into the surface's row-major control layout. Target
    /// multiplicity follows OpenNURBS and ranges from one through the V degree;
    /// an endpoint accepts only one (a no-op) or the degree (full clamping).
    pub fn try_insert_knot_v(
        &self,
        parameter: Real,
        target_multiplicity: usize,
    ) -> Result<Self, GeometryError> {
        validate_surface_knot_insertion(
            parameter,
            target_multiplicity,
            self.degree_v,
            self.domain_v(),
        )?;
        let restore_periodic_topology = self.insertion_curve_is_periodic_v();
        let mut columns = Vec::new();
        columns
            .try_reserve_exact(self.control_point_count_u)
            .map_err(|_| GeometryError::InvalidControlNet {
                context: "inserted V column list exceeds addressable memory",
            })?;
        let mut refined_v_count = None;
        let mut refined_knots_v = None;
        for u in 0..self.control_point_count_u {
            let column = (0..self.control_point_count_v)
                .map(|v| self.control_points[self.control_index(u, v)])
                .collect::<Vec<_>>();
            let column = NurbsCurve::try_new_rational(self.degree_v, column, self.knots_v.clone())?
                .try_insert_knot_with_periodic_topology(
                    parameter,
                    target_multiplicity,
                    restore_periodic_topology,
                )?;
            if refined_v_count.is_none() {
                refined_v_count = Some(column.control_points().len());
                refined_knots_v = Some(column.knots().to_vec());
            }
            debug_assert_eq!(refined_v_count, Some(column.control_points().len()));
            debug_assert_eq!(refined_knots_v.as_deref(), Some(column.knots()));
            columns.push(column);
        }

        let refined_v_count = refined_v_count.expect("a valid surface has at least one U column");
        let total = self
            .control_point_count_u
            .checked_mul(refined_v_count)
            .ok_or(GeometryError::InvalidControlNet {
                context: "inserted V control-net size overflowed usize",
            })?;
        let mut controls = Vec::new();
        controls
            .try_reserve_exact(total)
            .map_err(|_| GeometryError::InvalidControlNet {
                context: "inserted V control net exceeds addressable memory",
            })?;
        for v in 0..refined_v_count {
            controls.extend(columns.iter().map(|column| column.control_points()[v]));
        }

        Self::try_new_rational(
            self.degree_u,
            self.degree_v,
            self.control_point_count_u,
            refined_v_count,
            controls,
            self.knots_u.clone(),
            refined_knots_v.expect("a valid surface has a V knot vector"),
        )
    }

    /// Removes the U knot value nearest `parameter` and adjusts every fixed-V
    /// homogeneous control curve using Rhino's Greville-collocation rule.
    /// Midpoint ties select the higher knot value. A non-clamped U direction
    /// is clamped without changing its active domain before interpolation.
    pub fn try_remove_knot_u_near(&self, parameter: Real) -> Result<Self, GeometryError> {
        if self.is_periodic_u() {
            return Err(GeometryError::PeriodicKnotRemovalUnsupported {
                direction: "surface U direction",
            });
        }
        self.map_u_control_curves(|curve| {
            curve.try_remove_knot_near_parameter_with_periodic_topology(parameter, false)
        })
    }

    /// Removes the V knot value nearest `parameter` and adjusts every fixed-U
    /// homogeneous control curve using Rhino's Greville-collocation rule.
    /// Midpoint ties select the higher knot value. A non-clamped V direction
    /// is clamped without changing its active domain before interpolation.
    pub fn try_remove_knot_v_near(&self, parameter: Real) -> Result<Self, GeometryError> {
        if self.is_periodic_v() {
            return Err(GeometryError::PeriodicKnotRemovalUnsupported {
                direction: "surface V direction",
            });
        }
        self.map_v_control_curves(|curve| {
            curve.try_remove_knot_near_parameter_with_periodic_topology(parameter, false)
        })
    }

    /// Removes a complete control row in one parameter direction.
    ///
    /// Every orthogonal control curve uses [`NurbsCurve::try_remove_control_point`],
    /// so degree lowering, rational endpoint normalization, even-degree knot
    /// merging, and periodic topology match Rhino in both directions.
    pub fn try_remove_control_point(
        &self,
        direction: SurfaceKnotDirection,
        index: usize,
    ) -> Result<Self, GeometryError> {
        let (control_point_count, direction_name) = match direction {
            SurfaceKnotDirection::U => (
                if self.is_periodic_u() {
                    self.control_point_count_u - self.degree_u
                } else {
                    self.control_point_count_u
                },
                "surface U direction",
            ),
            SurfaceKnotDirection::V => (
                if self.is_periodic_v() {
                    self.control_point_count_v - self.degree_v
                } else {
                    self.control_point_count_v
                },
                "surface V direction",
            ),
            SurfaceKnotDirection::Both => {
                return Err(GeometryError::InvalidControlNet {
                    context: "control-point removal requires one surface direction",
                });
            }
        };
        if index >= control_point_count {
            return Err(GeometryError::ControlPointIndexOutOfRange {
                direction: direction_name,
                index,
                control_point_count,
            });
        }
        match direction {
            SurfaceKnotDirection::U => {
                self.map_u_control_curves(|curve| curve.try_remove_control_point(index))
            }
            SurfaceKnotDirection::V => {
                self.map_v_control_curves(|curve| curve.try_remove_control_point(index))
            }
            SurfaceKnotDirection::Both => unreachable!("Both was rejected above"),
        }
    }

    /// Collapses qualifying multiple knots in the selected parameter
    /// direction(s), using the same descending-knot interpolation order as
    /// [`NurbsCurve::try_remove_multiple_knots`].
    ///
    /// Fully multiple creases are filtered by the greatest one-sided tangent
    /// angle sampled at each start, midpoint, and end of every knot span in
    /// the other direction. This is the OpenNURBS surface-continuity sampling
    /// rule used by Rhino. Degree-one knots are removed completely. Selected
    /// periodic directions are rejected.
    pub fn try_remove_multiple_knots(
        &self,
        direction: SurfaceKnotDirection,
        remove_fully_multiple_knots: bool,
        maximum_kink_angle_radians: Real,
    ) -> Result<(Self, usize), GeometryError> {
        if !maximum_kink_angle_radians.is_finite()
            || !(0.0..=std::f64::consts::PI).contains(&maximum_kink_angle_radians)
        {
            return Err(GeometryError::InvalidKnotRemovalAngle);
        }
        if direction.includes_u() && self.is_periodic_u() {
            return Err(GeometryError::PeriodicKnotRemovalUnsupported {
                direction: "surface U direction",
            });
        }
        if direction.includes_v() && self.is_periodic_v() {
            return Err(GeometryError::PeriodicKnotRemovalUnsupported {
                direction: "surface V direction",
            });
        }

        let mut result = self.clone();
        let mut removed = 0;
        if direction.includes_u() {
            let removals = result.multiple_knot_removals(
                SurfaceKnotDirection::U,
                remove_fully_multiple_knots,
                maximum_kink_angle_radians,
            )?;
            removed += removals.iter().map(|(_, count)| *count).sum::<usize>();
            if !removals.is_empty() {
                result = result.map_u_control_curves(|curve| {
                    curve.try_remove_multiple_knot_groups(&removals)
                })?;
            }
        }
        if direction.includes_v() {
            let removals = result.multiple_knot_removals(
                SurfaceKnotDirection::V,
                remove_fully_multiple_knots,
                maximum_kink_angle_radians,
            )?;
            removed += removals.iter().map(|(_, count)| *count).sum::<usize>();
            if !removals.is_empty() {
                result = result.map_v_control_curves(|curve| {
                    curve.try_remove_multiple_knot_groups(&removals)
                })?;
            }
        }
        Ok((result, removed))
    }

    fn multiple_knot_removals(
        &self,
        direction: SurfaceKnotDirection,
        remove_fully_multiple_knots: bool,
        maximum_kink_angle_radians: Real,
    ) -> Result<Vec<(Real, usize)>, GeometryError> {
        let (degree, structural_curve) = match direction {
            SurfaceKnotDirection::U => (
                self.degree_u,
                NurbsCurve::try_new_rational(
                    self.degree_u,
                    self.control_points[..self.control_point_count_u].to_vec(),
                    self.knots_u.clone(),
                )?,
            ),
            SurfaceKnotDirection::V => (
                self.degree_v,
                NurbsCurve::try_new_rational(
                    self.degree_v,
                    (0..self.control_point_count_v)
                        .map(|v| self.control_points[self.control_index(0, v)])
                        .collect(),
                    self.knots_v.clone(),
                )?,
            ),
            SurfaceKnotDirection::Both => {
                unreachable!("multiple-knot removals inspect one surface direction at a time")
            }
        };

        let mut removals = Vec::new();
        for (knot, multiplicity) in structural_curve.interior_knot_groups() {
            let eligible = if remove_fully_multiple_knots {
                let is_multiple = multiplicity > 1 || degree == 1;
                if !is_multiple || multiplicity > degree {
                    false
                } else {
                    let kink_angle = if multiplicity < degree {
                        0.0
                    } else {
                        self.maximum_kink_angle_at(direction, knot)?
                    };
                    kink_angle < maximum_kink_angle_radians
                }
            } else {
                multiplicity > 1 && multiplicity < degree
            };
            if eligible {
                removals.push((
                    knot,
                    if degree == 1 {
                        multiplicity
                    } else {
                        multiplicity - 1
                    },
                ));
            }
        }
        Ok(removals)
    }

    fn maximum_kink_angle_at(
        &self,
        direction: SurfaceKnotDirection,
        knot: Real,
    ) -> Result<Real, GeometryError> {
        let parameters = match direction {
            SurfaceKnotDirection::U => surface_continuity_sample_parameters(
                self.degree_v,
                self.control_point_count_v,
                &self.knots_v,
            )?,
            SurfaceKnotDirection::V => surface_continuity_sample_parameters(
                self.degree_u,
                self.control_point_count_u,
                &self.knots_u,
            )?,
            SurfaceKnotDirection::Both => {
                unreachable!("kink angles inspect one surface direction at a time")
            }
        };
        let mut maximum: Real = 0.0;
        for parameter in parameters {
            let isocurve = match direction {
                SurfaceKnotDirection::U => self.isocurve_u(parameter)?,
                SurfaceKnotDirection::V => self.isocurve_v(parameter)?,
                SurfaceKnotDirection::Both => unreachable!(),
            };
            maximum = maximum.max(isocurve.kink_angle_at(knot)?);
        }
        Ok(maximum)
    }

    /// Matches the high-dimensional non-rational curve that OpenNURBS uses
    /// internally for surface U insertion. Rational controls are compared in
    /// their stored homogeneous form, including their weights.
    fn insertion_curve_is_periodic_u(&self) -> bool {
        knot_vector_is_periodic(
            self.degree_u + 1,
            self.control_point_count_u,
            &self.knots_u[1..self.knots_u.len() - 1],
        ) && (0..self.control_point_count_v).all(|v| {
            (0..self.degree_u).all(|u| {
                let repeated = self.control_point_count_u - self.degree_u + u;
                homogeneous_controls_coincident(
                    self.control_points[self.control_index(u, v)],
                    self.control_points[self.control_index(repeated, v)],
                )
            })
        })
    }

    /// V-direction counterpart of [`Self::insertion_curve_is_periodic_u`].
    fn insertion_curve_is_periodic_v(&self) -> bool {
        knot_vector_is_periodic(
            self.degree_v + 1,
            self.control_point_count_v,
            &self.knots_v[1..self.knots_v.len() - 1],
        ) && (0..self.control_point_count_u).all(|u| {
            (0..self.degree_v).all(|v| {
                let repeated = self.control_point_count_v - self.degree_v + v;
                homogeneous_controls_coincident(
                    self.control_points[self.control_index(u, v)],
                    self.control_points[self.control_index(u, repeated)],
                )
            })
        })
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

    /// Extracts the exact U-direction isocurve at a fixed V parameter.
    ///
    /// The returned curve retains the surface's complete U knot vector and
    /// degree. Its homogeneous controls are obtained by evaluating every
    /// control-net column in V, so this also works at non-clamped and periodic
    /// parameter values where copying a control row would be incorrect.
    pub fn isocurve_u(&self, v: Real) -> Result<crate::NurbsCurve, GeometryError> {
        let span_v = checked_span(self.degree_v, self.control_point_count_v, &self.knots_v, v)?;
        let controls = self.isocurve_controls(
            SurfaceIsoDirection::U,
            span_v,
            v,
            self.degree_v,
            &self.knots_v,
        )?;
        crate::NurbsCurve::try_new_rational(self.degree_u, controls, self.knots_u.clone())
    }

    /// Extracts the exact V-direction isocurve at a fixed U parameter.
    ///
    /// The returned curve retains the surface's complete V knot vector and
    /// degree. Its homogeneous controls are obtained by evaluating every
    /// control-net row in U, including for non-clamped and periodic surfaces.
    pub fn isocurve_v(&self, u: Real) -> Result<crate::NurbsCurve, GeometryError> {
        let span_u = checked_span(self.degree_u, self.control_point_count_u, &self.knots_u, u)?;
        let controls = self.isocurve_controls(
            SurfaceIsoDirection::V,
            span_u,
            u,
            self.degree_u,
            &self.knots_u,
        )?;
        crate::NurbsCurve::try_new_rational(self.degree_v, controls, self.knots_v.clone())
    }

    /// Splits this tensor surface at a parameter strictly inside its U domain.
    /// Both results preserve V data and source parameter values and are
    /// clamped in U at every active end.
    pub fn try_split_u(&self, u: Real) -> Result<(Self, Self), GeometryError> {
        let left = self.map_u_control_curves(|curve| Ok(curve.try_split(u)?.0))?;
        let right = self.map_u_control_curves(|curve| Ok(curve.try_split(u)?.1))?;
        Ok((left, right))
    }

    /// Splits this tensor surface at a parameter strictly inside its V domain.
    /// Both results preserve U data and source parameter values and are
    /// clamped in V at every active end.
    pub fn try_split_v(&self, v: Real) -> Result<(Self, Self), GeometryError> {
        let low = self.map_v_control_curves(|curve| Ok(curve.try_split(v)?.0))?;
        let high = self.map_v_control_curves(|curve| Ok(curve.try_split(v)?.1))?;
        Ok((low, high))
    }

    /// Restricts the active U domain without changing surface parameterization
    /// or the retained geometric image. A full-domain trim is a no-op and
    /// therefore preserves periodic form.
    pub fn try_trimmed_u(&self, interval: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        self.map_u_control_curves(|curve| curve.try_trimmed(interval.clone()))
    }

    /// Restricts the active V domain without changing surface parameterization
    /// or the retained geometric image. A full-domain trim is a no-op and
    /// therefore preserves periodic form.
    pub fn try_trimmed_v(&self, interval: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        self.map_v_control_curves(|curve| curve.try_trimmed(interval.clone()))
    }

    /// Extracts an exact rectangular subdomain of this tensor surface.
    pub fn try_trimmed(
        &self,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
    ) -> Result<Self, GeometryError> {
        self.try_trimmed_u(u)?.try_trimmed_v(v)
    }

    /// Returns whether the natural U direction closes without a border.
    pub fn is_closed_u(&self) -> Result<bool, GeometryError> {
        if self.is_periodic_u() {
            return Ok(true);
        }
        let domain = self.domain_u();
        let start = self.isocurve_v(*domain.start())?;
        let end = self.isocurve_v(*domain.end())?;
        Ok(isocurve_controls_coincident(&start, &end))
    }

    /// Returns whether the natural V direction closes without a border.
    pub fn is_closed_v(&self) -> Result<bool, GeometryError> {
        if self.is_periodic_v() {
            return Ok(true);
        }
        let domain = self.domain_v();
        let start = self.isocurve_u(*domain.start())?;
        let end = self.isocurve_u(*domain.end())?;
        Ok(isocurve_controls_coincident(&start, &end))
    }

    /// Extracts every non-degenerate natural border as exact NURBS curves.
    ///
    /// Each inner vector is one connected border. A rectangular open patch
    /// therefore has four perimeter-ordered curves, a cylinder has two
    /// one-curve circular borders, and a surface closed in both directions has
    /// none. Singular collapsed sides, such as a cone apex or sphere pole, are
    /// omitted because they are points rather than curve borders.
    pub fn natural_boundary_curve_loops(
        &self,
    ) -> Result<Vec<Vec<crate::NurbsCurve>>, GeometryError> {
        let closed_u = self.is_closed_u()?;
        let closed_v = self.is_closed_v()?;
        if closed_u && closed_v {
            return Ok(Vec::new());
        }

        let u_domain = self.domain_u();
        let v_domain = self.domain_v();
        if !closed_u && !closed_v {
            let candidates = [
                self.isocurve_u(*v_domain.start())?,
                self.isocurve_v(*u_domain.end())?,
                self.isocurve_u(*v_domain.end())?.reversed()?,
                self.isocurve_v(*u_domain.start())?.reversed()?,
            ];
            let perimeter = candidates
                .into_iter()
                .filter(curve_has_extent)
                .collect::<Vec<_>>();
            return Ok((!perimeter.is_empty())
                .then_some(perimeter)
                .into_iter()
                .collect());
        }

        let candidates = if closed_u {
            vec![
                self.isocurve_u(*v_domain.start())?,
                self.isocurve_u(*v_domain.end())?.reversed()?,
            ]
        } else {
            vec![
                self.isocurve_v(*u_domain.end())?,
                self.isocurve_v(*u_domain.start())?.reversed()?,
            ]
        };
        Ok(candidates
            .into_iter()
            .filter(curve_has_extent)
            .map(|curve| vec![curve])
            .collect())
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

    /// Builds Rhino's cleaned polygon mesh through the Euclidean control net.
    ///
    /// Regular cells remain quads. Cells beside a singular side become
    /// triangles, and invalid collapsed cells are omitted before unused
    /// vertices are culled. Closed directions retain one coincident seam row
    /// or column; periodic directions use their domain-aligned Greville
    /// window rather than the raw repeated-control prefix.
    pub fn control_polygon_mesh(
        &self,
        tolerance: Tolerance,
    ) -> Result<Option<TriangleMesh>, GeometryError> {
        let periodic_u = self.is_periodic_u();
        let periodic_v = self.is_periodic_v();
        let (start_u, end_u) = control_polygon_range(
            self.degree_u,
            self.control_point_count_u,
            &self.knots_u,
            periodic_u,
        );
        let (start_v, end_v) = control_polygon_range(
            self.degree_v,
            self.control_point_count_v,
            &self.knots_v,
            periodic_v,
        );
        let count_u = end_u - start_u;
        let count_v = end_v - start_v;
        let vertex_count = count_u
            .checked_mul(count_v)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if vertex_count
            .checked_sub(1)
            .is_some_and(|last| u32::try_from(last).is_err())
        {
            return Err(GeometryError::TooManyMeshVertices);
        }
        let face_capacity = count_u
            .checked_sub(1)
            .and_then(|u| count_v.checked_sub(1).and_then(|v| u.checked_mul(v)))
            .ok_or(GeometryError::TooManyMeshFaces)?;

        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(vertex_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        for v in start_v..end_v {
            for u in start_u..end_u {
                vertices.push(self.control_points[self.control_index(u, v)].point());
            }
        }

        let singular_sides = control_net_singular_sides(
            &vertices,
            count_u,
            count_v,
            [
                knots_are_clamped_at_start(self.degree_v, &self.knots_v),
                knots_are_clamped_at_end(self.degree_u, &self.knots_u),
                knots_are_clamped_at_end(self.degree_v, &self.knots_v),
                knots_are_clamped_at_start(self.degree_u, &self.knots_u),
            ],
        );
        if self.is_closed_u()? {
            for v in 0..count_v {
                let first = vertices[v * count_u];
                vertices[v * count_u + count_u - 1] = first;
            }
        }
        if self.is_closed_v()? {
            let last_row = count_u * (count_v - 1);
            for u in 0..count_u {
                vertices[last_row + u] = vertices[u];
            }
        }
        snap_singular_control_net_sides(&mut vertices, count_u, count_v, singular_sides);

        let mut faces = Vec::new();
        faces
            .try_reserve_exact(face_capacity)
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        let mut omitted_face = false;
        for v in 1..count_v {
            for u in 1..count_u {
                let current = v * count_u + u;
                let raw = [
                    current - count_u - 1,
                    current - count_u,
                    current,
                    current - 1,
                ]
                .map(|index| index as u32);
                match clean_control_net_face(raw, &vertices) {
                    Some(face) => faces.push(face),
                    None => omitted_face = true,
                }
            }
        }
        if faces.is_empty() {
            return Ok(None);
        }
        let mesh = TriangleMesh::try_new_faces(vertices, faces, tolerance)?;
        Ok(Some(if omitted_face {
            mesh.culled_unused_vertices().0
        } else {
            mesh
        }))
    }

    fn isocurve_controls(
        &self,
        direction: SurfaceIsoDirection,
        fixed_span: usize,
        fixed_parameter: Real,
        fixed_degree: usize,
        fixed_knots: &[Real],
    ) -> Result<Vec<WeightedPoint3>, GeometryError> {
        let first_fixed = fixed_span - fixed_degree;
        let varying_count = match direction {
            SurfaceIsoDirection::U => self.control_point_count_u,
            SurfaceIsoDirection::V => self.control_point_count_v,
        };
        let mut result = Vec::with_capacity(varying_count);
        for varying in 0..varying_count {
            let control_at = |fixed| match direction {
                SurfaceIsoDirection::U => self.control_points[self.control_index(varying, fixed)],
                SurfaceIsoDirection::V => self.control_points[self.control_index(fixed, varying)],
            };
            let weight_scale = (0..=fixed_degree)
                .map(|local_fixed| control_at(first_fixed + local_fixed).weight().abs())
                .fold(0.0_f64, Real::max);
            debug_assert!(weight_scale > 0.0);
            let mut active = Vec::with_capacity(fixed_degree + 1);
            for local_fixed in 0..=fixed_degree {
                let fixed = first_fixed + local_fixed;
                let control = control_at(fixed);
                let weight = control.weight() / weight_scale;
                let point = control.point();
                let homogeneous = [
                    point.x() * weight,
                    point.y() * weight,
                    point.z() * weight,
                    weight,
                ];
                require_finite(homogeneous, "homogeneous NURBS isocurve control point")?;
                active.push(homogeneous);
            }
            let homogeneous = de_boor(
                fixed_knots,
                fixed_degree,
                fixed_span,
                fixed_parameter,
                active,
            )?;
            let weight = homogeneous[3] * weight_scale;
            require_finite([weight], "NURBS isocurve weight")?;
            result.push(WeightedPoint3::try_new(
                project_homogeneous(homogeneous)?,
                weight,
            )?);
        }
        Ok(result)
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

    /// Returns all U parameters used by an OpenNURBS-compatible wireframe.
    /// Natural boundaries are included even when they form a closed seam.
    pub fn wire_parameters_u(&self, wire_density: i32) -> Result<Vec<Real>, GeometryError> {
        surface_wire_parameters(self.spans_u(), wire_density)
    }

    /// Returns all V parameters used by an OpenNURBS-compatible wireframe.
    /// Natural boundaries are included even when they form a closed seam.
    pub fn wire_parameters_v(&self, wire_density: i32) -> Result<Vec<Real>, GeometryError> {
        surface_wire_parameters(self.spans_v(), wire_density)
    }

    /// Returns the standalone surface's exact topological edges in
    /// OpenNURBS order.
    ///
    /// Closed directions contribute one seam rather than two coincident
    /// sides, while collapsed singular sides are omitted. Consequently an
    /// open patch has four edges, a cylinder has two rims and one seam, a
    /// sphere has one seam, and a torus has two seams.
    pub fn natural_edge_curves(&self) -> Result<Vec<crate::NurbsCurve>, GeometryError> {
        let closed_u = self.is_closed_u()?;
        let closed_v = self.is_closed_v()?;
        let u_start = *self.domain_u().start();
        let u_end = *self.domain_u().end();
        let v_start = *self.domain_v().start();
        let v_end = *self.domain_v().end();
        let mut curves = Vec::new();

        match (closed_u, closed_v) {
            (false, false) => {
                push_surface_wire(&mut curves, self.isocurve_u(v_start)?)?;
                push_surface_wire(&mut curves, self.isocurve_v(u_end)?)?;
                push_surface_wire(&mut curves, self.isocurve_u(v_end)?.reversed()?)?;
                push_surface_wire(&mut curves, self.isocurve_v(u_start)?.reversed()?)?;
            }
            (true, false) => {
                push_surface_wire(&mut curves, self.isocurve_u(v_start)?)?;
                push_surface_wire(&mut curves, self.isocurve_v(u_start)?)?;
                push_surface_wire(&mut curves, self.isocurve_u(v_end)?.reversed()?)?;
            }
            (false, true) => {
                push_surface_wire(&mut curves, self.isocurve_v(u_start)?)?;
                push_surface_wire(&mut curves, self.isocurve_u(v_start)?)?;
                push_surface_wire(&mut curves, self.isocurve_v(u_end)?.reversed()?)?;
            }
            (true, true) => {
                push_surface_wire(&mut curves, self.isocurve_v(u_start)?)?;
                push_surface_wire(&mut curves, self.isocurve_u(v_start)?)?;
            }
        }

        Ok(curves)
    }

    /// Returns the exact topological boundaries, seams, and interior
    /// isoparametric curves displayed for this standalone surface.
    pub fn wireframe_curves(
        &self,
        wire_density: i32,
    ) -> Result<Vec<crate::NurbsCurve>, GeometryError> {
        let parameters_u = self.wire_parameters_u(wire_density)?;
        let parameters_v = self.wire_parameters_v(wire_density)?;
        let mut curves = self.natural_edge_curves()?;

        for v in interior_wire_parameters(&parameters_v) {
            push_surface_wire(&mut curves, self.isocurve_u(v)?)?;
        }
        for u in interior_wire_parameters(&parameters_u) {
            push_surface_wire(&mut curves, self.isocurve_v(u)?)?;
        }
        Ok(curves)
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

    /// Computes the area of the complete natural surface domain directly
    /// from the exact NURBS representation.
    ///
    /// Every nonempty knot-span rectangle is integrated independently. The
    /// control net is recentered before derivative evaluation so large model
    /// translations do not degrade the result through rational-coordinate
    /// cancellation.
    pub fn area(&self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        let bounds = self.control_point_bounds();
        let reference = bounds.center()?;
        let scale = bounds.min().distance_to(bounds.max())?;
        let absolute_tolerance = match product_three(
            tolerance.absolute(),
            scale.max(tolerance.absolute()),
            1.0,
            "NURBS surface area tolerance",
        ) {
            Ok(value) => value,
            // An unrepresentably large tolerance places no useful absolute
            // restriction on the adaptive rule; its relative target remains.
            Err(GeometryError::NonFinite { .. }) => Real::MAX,
            Err(error) => return Err(error),
        };
        let centered = self.centered(reference)?;
        let spans_u = centered.spans_u().collect::<Vec<_>>();
        let spans_v = centered.spans_v().collect::<Vec<_>>();
        let patch_count = spans_u
            .len()
            .checked_mul(spans_v.len())
            .ok_or(GeometryError::NumericalIntegrationDidNotConverge)?;
        let patch_tolerance = (absolute_tolerance / patch_count as Real).max(Real::MIN_POSITIVE);
        let mut sum = 0.0;
        let mut correction = 0.0;
        for &(u_start, u_end) in &spans_u {
            for &(v_start, v_end) in &spans_v {
                let contribution = integrate_area_patch(
                    &centered,
                    [u_start, u_end],
                    [v_start, v_end],
                    patch_tolerance,
                    tolerance.relative(),
                )?;
                compensated_add(&mut sum, &mut correction, contribution);
            }
        }
        let area = sum + correction;
        require_finite([area], "NURBS surface area")?;
        Ok(area)
    }

    fn centered(&self, reference: Point3) -> Result<Self, GeometryError> {
        let control_points = self
            .control_points
            .iter()
            .map(|control| {
                let point = control.point();
                WeightedPoint3::try_new(
                    Point3::try_new(
                        point.x() - reference.x(),
                        point.y() - reference.y(),
                        point.z() - reference.z(),
                    )?,
                    control.weight(),
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

    /// Returns the surface plane when the exact rational control net and
    /// sampled differential orientation are planar at model tolerance.
    ///
    /// Testing the control net is sufficient to prove that the full rational
    /// surface lies in the plane. Mid-span derivatives additionally reject a
    /// folded or orientation-reversing parameterization.
    pub fn plane(&self, tolerance: Tolerance) -> Result<Option<Plane>, GeometryError> {
        let mut derivative_crosses = Vec::new();
        let mut largest = None;
        let mut largest_area = 0.0;
        for (u_start, u_end) in self.spans_u() {
            let u = u_start * 0.5 + u_end * 0.5;
            for (v_start, v_end) in self.spans_v() {
                let v = v_start * 0.5 + v_end * 0.5;
                let (point, derivative_u, derivative_v) = self.evaluate_with_derivatives(u, v)?;
                let cross = derivative_u.cross(derivative_v)?;
                let area = cross.length()?;
                if area > 0.0 {
                    derivative_crosses.push(cross);
                    if area > largest_area {
                        largest_area = area;
                        largest = Some((point, cross));
                    }
                }
            }
        }
        let Some((origin, cross)) = largest else {
            return Ok(None);
        };
        let normal = cross.normalized_nonzero()?;
        for sample in derivative_crosses {
            let length = sample.length()?;
            if sample.dot(normal.as_vector())? <= tolerance.angular() * length {
                return Ok(None);
            }
        }
        for control in &self.control_points {
            let point = control.point();
            let distance = origin.vector_to(point)?.dot(normal.as_vector())?.abs();
            let coordinate_scale = origin
                .to_array()
                .into_iter()
                .chain(point.to_array())
                .map(Real::abs)
                .fold(0.0, Real::max);
            let allowed = tolerance
                .absolute()
                .max(tolerance.relative() * coordinate_scale);
            if distance > allowed {
                return Ok(None);
            }
        }
        Ok(Some(Plane::new(origin, normal)))
    }

    /// Produces a regular display mesh inside every nonempty knot-span pair.
    /// Span boundaries are sampled independently so a fully multiple knot
    /// cannot bridge a discontinuity. Boundary samples that meet within model
    /// tolerance are made exactly coincident. Singular triangles, such as the
    /// collapsed row at a sphere pole, are omitted.
    pub fn tessellate(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        self.tessellate_grid(samples_per_span, false, tolerance)
    }

    /// Creates an editable triangle/quad mesh using Rhino-style normalized
    /// density. Regular parameter cells remain quadrilaterals, while cells at
    /// singular surface sides become triangles. A single degree-one span uses
    /// its control quadrilateral, matching Rhino's common coarse-surface case.
    pub fn polygon_mesh(
        &self,
        density: Real,
        simple_planes: bool,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        let samples_per_span =
            self.polygon_mesh_samples_per_span(density, simple_planes, tolerance)?;
        self.tessellate_grid(samples_per_span, true, tolerance)
    }

    pub(crate) fn polygon_mesh_samples_per_span(
        &self,
        density: Real,
        simple_planes: bool,
        tolerance: Tolerance,
    ) -> Result<usize, GeometryError> {
        if !density.is_finite() || !(0.0..=1.0).contains(&density) {
            return Err(GeometryError::InvalidMeshDensity(density));
        }
        if (self.degree_u == 1 && self.degree_v == 1)
            || (simple_planes && self.plane(tolerance)?.is_some())
        {
            return Ok(1);
        }

        // Four samples at density zero matches Rhino's coarse curved-surface
        // floor. Three binary refinement levels reach 32 samples per knot
        // span at density one without permitting unbounded allocations.
        let refinement_level = (density * 3.0).round() as u32;
        Ok(4_usize << refinement_level)
    }

    pub(crate) fn tessellate_grid(
        &self,
        samples_per_span: usize,
        preserve_quads: bool,
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
        let mut faces = Vec::new();
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
                        push_tessellation_cell(
                            &vertices,
                            &mut faces,
                            [lower_left, lower_right, upper_right, upper_left],
                            preserve_quads,
                            tolerance,
                        )?;
                    }
                }
            }
        }
        stitch_continuous_patch_boundaries(
            &mut vertices,
            spans_u.len(),
            spans_v.len(),
            samples_per_span,
            tolerance,
        )?;
        TriangleMesh::try_new_faces(vertices, faces, tolerance)
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
                        .weight()
                        .abs(),
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

    fn map_u_control_curves(
        &self,
        mut map: impl FnMut(&crate::NurbsCurve) -> Result<crate::NurbsCurve, GeometryError>,
    ) -> Result<Self, GeometryError> {
        let mut controls = Vec::new();
        let mut output_knots = None;
        let mut output_count = None;
        let mut output_degree = None;
        for row in self.control_points.chunks_exact(self.control_point_count_u) {
            let curve = crate::NurbsCurve::try_new_rational(
                self.degree_u,
                row.to_vec(),
                self.knots_u.clone(),
            )?;
            let mapped = map(&curve)?;
            if output_degree.is_some_and(|degree| degree != mapped.degree())
                || output_count.is_some_and(|count| count != mapped.control_points().len())
                || output_knots
                    .as_ref()
                    .is_some_and(|knots| knots != mapped.knots())
            {
                return Err(GeometryError::InvalidControlNet {
                    context: "U control curves produced inconsistent mapped layouts",
                });
            }
            output_degree.get_or_insert(mapped.degree());
            if output_count.is_none() {
                let control_count = mapped
                    .control_points()
                    .len()
                    .checked_mul(self.control_point_count_v)
                    .ok_or(GeometryError::InvalidControlNet {
                        context: "mapped surface control-point count overflowed usize",
                    })?;
                controls.try_reserve_exact(control_count).map_err(|_| {
                    GeometryError::InvalidControlNet {
                        context: "mapped surface control net exceeds addressable memory",
                    }
                })?;
            }
            output_count.get_or_insert(mapped.control_points().len());
            output_knots.get_or_insert_with(|| mapped.knots().to_vec());
            controls.extend_from_slice(mapped.control_points());
        }
        let control_point_count_u = output_count.ok_or(GeometryError::InvalidControlNet {
            context: "a surface has no U control curves",
        })?;
        let degree_u = output_degree.ok_or(GeometryError::InvalidControlNet {
            context: "a surface has no U control-curve degree",
        })?;
        let knots_u = output_knots.ok_or(GeometryError::InvalidControlNet {
            context: "a surface has no U control-curve knot vector",
        })?;
        Self::try_new_rational(
            degree_u,
            self.degree_v,
            control_point_count_u,
            self.control_point_count_v,
            controls,
            knots_u,
            self.knots_v.clone(),
        )
    }

    fn map_v_control_curves(
        &self,
        mut map: impl FnMut(&crate::NurbsCurve) -> Result<crate::NurbsCurve, GeometryError>,
    ) -> Result<Self, GeometryError> {
        let mut columns = Vec::with_capacity(self.control_point_count_u);
        let mut output_knots = None;
        let mut output_count = None;
        let mut output_degree = None;
        for u in 0..self.control_point_count_u {
            let column = (0..self.control_point_count_v)
                .map(|v| self.control_points[self.control_index(u, v)])
                .collect::<Vec<_>>();
            let curve =
                crate::NurbsCurve::try_new_rational(self.degree_v, column, self.knots_v.clone())?;
            let mapped = map(&curve)?;
            if output_degree.is_some_and(|degree| degree != mapped.degree())
                || output_count.is_some_and(|count| count != mapped.control_points().len())
                || output_knots
                    .as_ref()
                    .is_some_and(|knots| knots != mapped.knots())
            {
                return Err(GeometryError::InvalidControlNet {
                    context: "V control curves produced inconsistent mapped layouts",
                });
            }
            output_degree.get_or_insert(mapped.degree());
            output_count.get_or_insert(mapped.control_points().len());
            output_knots.get_or_insert_with(|| mapped.knots().to_vec());
            columns.push(mapped.control_points().to_vec());
        }
        let control_point_count_v = output_count.ok_or(GeometryError::InvalidControlNet {
            context: "a surface has no V control curves",
        })?;
        let degree_v = output_degree.ok_or(GeometryError::InvalidControlNet {
            context: "a surface has no V control-curve degree",
        })?;
        let knots_v = output_knots.ok_or(GeometryError::InvalidControlNet {
            context: "a surface has no V control-curve knot vector",
        })?;
        let control_count = self
            .control_point_count_u
            .checked_mul(control_point_count_v)
            .ok_or(GeometryError::InvalidControlNet {
                context: "mapped surface control-point count overflowed usize",
            })?;
        let mut controls = Vec::new();
        controls.try_reserve_exact(control_count).map_err(|_| {
            GeometryError::InvalidControlNet {
                context: "mapped surface control net exceeds addressable memory",
            }
        })?;
        for v in 0..control_point_count_v {
            for column in &columns {
                controls.push(column[v]);
            }
        }
        Self::try_new_rational(
            self.degree_u,
            degree_v,
            self.control_point_count_u,
            control_point_count_v,
            controls,
            self.knots_u.clone(),
            knots_v,
        )
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

fn surface_continuity_sample_parameters(
    degree: usize,
    control_point_count: usize,
    knots: &[Real],
) -> Result<Vec<Real>, GeometryError> {
    let mut parameters = vec![knots[degree]];
    for index in degree..control_point_count {
        parameters.push(stable_knot_mean(&[knots[index], knots[index + 1]])?);
        parameters.push(knots[index + 1]);
    }
    Ok(parameters)
}

fn surface_wire_parameters(
    spans: impl Iterator<Item = (Real, Real)>,
    wire_density: i32,
) -> Result<Vec<Real>, GeometryError> {
    if !(crate::MIN_SURFACE_WIRE_DENSITY..=crate::MAX_SURFACE_WIRE_DENSITY).contains(&wire_density)
    {
        return Err(GeometryError::InvalidSurfaceWireDensity(wire_density));
    }

    // Rhino/OpenNURBS density -1 draws only natural boundaries, 0 adds knot
    // wires, 1 adds one midpoint only when there are no interior knots, and
    // N >= 2 adds N-1 evenly spaced wires inside every nonempty knot span.
    let spans = spans.collect::<Vec<_>>();
    let first = spans
        .first()
        .copied()
        .expect("a validated NURBS direction has a nonempty span");
    let last = spans
        .last()
        .copied()
        .expect("a validated NURBS direction has a nonempty span");
    if wire_density < 0 {
        return Ok(vec![first.0, last.1]);
    }
    let extra_per_span = match wire_density {
        1 if spans.len() == 1 => 1,
        2.. => (wire_density - 1) as usize,
        _ => 0,
    };
    let parameter_count = spans
        .len()
        .checked_mul(extra_per_span + 1)
        .and_then(|count| count.checked_add(1))
        .filter(|&count| count <= crate::MAX_SURFACE_WIRES)
        .ok_or(GeometryError::TooManySurfaceWires)?;
    let mut parameters = Vec::new();
    parameters
        .try_reserve_exact(parameter_count)
        .map_err(|_| GeometryError::TooManySurfaceWires)?;
    parameters.push(first.0);
    for (start, end) in spans {
        for division in 1..=extra_per_span {
            let fraction = division as Real / (extra_per_span + 1) as Real;
            let parameter = start.mul_add(1.0 - fraction, end * fraction);
            if parameter > start && parameter < end {
                parameters.push(parameter);
            }
        }
        parameters.push(end);
    }
    Ok(parameters)
}

fn interior_wire_parameters(parameters: &[Real]) -> impl Iterator<Item = Real> + '_ {
    parameters
        .iter()
        .copied()
        .skip(1)
        .take(parameters.len().saturating_sub(2))
}

fn push_surface_wire(
    curves: &mut Vec<crate::NurbsCurve>,
    curve: crate::NurbsCurve,
) -> Result<(), GeometryError> {
    if !curve_has_extent(&curve) {
        return Ok(());
    }
    if curves.len() == crate::MAX_SURFACE_WIRES {
        return Err(GeometryError::TooManySurfaceWires);
    }
    curves.push(curve);
    Ok(())
}

fn curve_has_extent(curve: &crate::NurbsCurve) -> bool {
    let first = curve.control_points()[0].point();
    curve
        .control_points()
        .iter()
        .any(|control| control.point() != first)
}

fn control_net_singular_sides(
    vertices: &[Point3],
    count_u: usize,
    count_v: usize,
    clamped: [bool; 4],
) -> [bool; 4] {
    let south =
        clamped[0] && (1..count_u).all(|u| curve_points_coincident(vertices[0], vertices[u]));
    let east_start = count_u - 1;
    let east = clamped[1]
        && (1..count_v).all(|v| {
            curve_points_coincident(vertices[east_start], vertices[v * count_u + east_start])
        });
    let north_start = (count_v - 1) * count_u;
    let north = clamped[2]
        && (1..count_u)
            .all(|u| curve_points_coincident(vertices[north_start], vertices[north_start + u]));
    let west = clamped[3]
        && (1..count_v).all(|v| curve_points_coincident(vertices[0], vertices[v * count_u]));
    [south, east, north, west]
}

fn snap_singular_control_net_sides(
    vertices: &mut [Point3],
    count_u: usize,
    count_v: usize,
    singular: [bool; 4],
) {
    if singular[0] {
        let point = vertices[0];
        vertices[..count_u].fill(point);
    }
    if singular[1] {
        let index = count_u - 1;
        let point = vertices[index];
        for v in 1..count_v {
            vertices[v * count_u + index] = point;
        }
    }
    if singular[2] {
        let start = (count_v - 1) * count_u;
        let point = vertices[start];
        vertices[start..start + count_u].fill(point);
    }
    if singular[3] {
        let point = vertices[0];
        for v in 1..count_v {
            vertices[v * count_u] = point;
        }
    }
}

/// Mirrors the `bCleanMesh` face cleanup in OpenNURBS'
/// `ON_ControlPolygonMesh`, including which duplicated pole index survives.
fn clean_control_net_face(mut indices: [u32; 4], vertices: &[Point3]) -> Option<MeshFace> {
    let mut points = indices.map(|index| vertices[index as usize]);
    if points[0] == points[1] {
        indices[1] = indices[2];
        indices[2] = indices[3];
        points[1] = points[2];
        points[2] = points[3];
    }
    if points[1] == points[2] {
        indices[2] = indices[3];
        points[2] = points[3];
    }
    if points[2] == points[3] {
        indices[2] = indices[3];
        points[2] = points[3];
    }
    if points[3] == points[0] {
        indices[0] = indices[1];
        indices[1] = indices[2];
        indices[2] = indices[3];
        points[0] = points[1];
        points[1] = points[2];
        points[2] = points[3];
    }
    if indices[0] == indices[1]
        || indices[1] == indices[2]
        || indices[3] == indices[0]
        || points[0] == points[2]
        || points[1] == points[3]
    {
        None
    } else if indices[2] == indices[3] {
        Some(MeshFace::Triangle([indices[0], indices[1], indices[2]]))
    } else {
        Some(MeshFace::Quad(indices))
    }
}

fn isocurve_controls_coincident(left: &crate::NurbsCurve, right: &crate::NurbsCurve) -> bool {
    if left.degree() != right.degree()
        || left.knots() != right.knots()
        || left.control_points().len() != right.control_points().len()
    {
        return false;
    }
    let left_scale = left
        .control_points()
        .iter()
        .map(|control| control.weight().abs())
        .fold(0.0_f64, Real::max);
    let right_scale = right
        .control_points()
        .iter()
        .map(|control| control.weight().abs())
        .fold(0.0_f64, Real::max);
    left.control_points()
        .iter()
        .zip(right.control_points())
        .all(|(left, right)| {
            let left_weight = left.weight() / left_scale;
            let right_weight = right.weight() / right_scale;
            curve_points_coincident(left.point(), right.point())
                && (left_weight - right_weight).abs()
                    <= 64.0 * Real::EPSILON * left_weight.abs().max(right_weight.abs()).max(1.0)
        })
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

pub(crate) fn integrate_area_patch(
    surface: &NurbsSurface,
    u: [Real; 2],
    v: [Real; 2],
    absolute_tolerance: Real,
    relative_tolerance: Real,
) -> Result<Real, GeometryError> {
    let half_u = u[1] * 0.5 - u[0] * 0.5;
    let half_v = v[1] * 0.5 - v[0] * 0.5;
    require_finite([half_u, half_v], "NURBS surface area parameter span")?;
    if half_u <= 0.0 || half_v <= 0.0 {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    let inner_tolerance = (absolute_tolerance * 0.25).max(Real::MIN_POSITIVE);
    integrate_adaptive(
        0.0,
        1.0,
        absolute_tolerance,
        relative_tolerance,
        |normalized_u| {
            let parameter_u = normalized_surface_span_parameter(u, normalized_u)?;
            integrate_adaptive(
                0.0,
                1.0,
                inner_tolerance,
                relative_tolerance,
                |normalized_v| {
                    let parameter_v = normalized_surface_span_parameter(v, normalized_v)?;
                    let (_, derivative_u, derivative_v) =
                        surface.evaluate_with_derivatives(parameter_u, parameter_v)?;
                    let normalized_u = derivative_u.scaled(half_u)?;
                    let normalized_v = derivative_v.scaled(half_v)?;
                    let jacobian = normalized_u.cross(normalized_v)?.length()?;
                    product_three(jacobian, 4.0, 1.0, "NURBS surface area integrand")
                },
            )
        },
    )
}

fn normalized_surface_span_parameter(
    span: [Real; 2],
    normalized: Real,
) -> Result<Real, GeometryError> {
    let parameter = span[0].mul_add(1.0 - normalized, span[1] * normalized);
    require_finite([parameter], "NURBS surface area parameter")?;
    Ok(parameter)
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
    knots_are_clamped_at_start(degree, knots) && knots_are_clamped_at_end(degree, knots)
}

fn knots_are_clamped_at_start(degree: usize, knots: &[Real]) -> bool {
    // OpenNURBS omits our two superfluous end knots. This is the equivalent
    // `ON_IsKnotVectorClamped(order, count, knots, 0)` comparison.
    knots[1] == knots[degree]
}

fn knots_are_clamped_at_end(degree: usize, knots: &[Real]) -> bool {
    let control_count = knots.len() - degree - 1;
    knots[control_count] == knots[knots.len() - 2]
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

fn stitch_continuous_patch_boundaries(
    vertices: &mut [Point3],
    span_count_u: usize,
    span_count_v: usize,
    samples_per_span: usize,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    let side = samples_per_span
        .checked_add(1)
        .ok_or(GeometryError::TooManyMeshVertices)?;
    let vertices_per_patch = side
        .checked_mul(side)
        .ok_or(GeometryError::TooManyMeshVertices)?;
    let patch_offset =
        |u_span: usize, v_span: usize| (v_span * span_count_u + u_span) * vertices_per_patch;

    for v_span in 0..span_count_v {
        for u_span in 0..span_count_u.saturating_sub(1) {
            let left = patch_offset(u_span, v_span);
            let right = patch_offset(u_span + 1, v_span);
            for v_sample in 0..=samples_per_span {
                let left_index = left + v_sample * side + samples_per_span;
                let right_index = right + v_sample * side;
                snap_first_point_to_second_if_near(vertices, left_index, right_index, tolerance)?;
            }
        }
    }
    for v_span in 0..span_count_v.saturating_sub(1) {
        for u_span in 0..span_count_u {
            let lower = patch_offset(u_span, v_span);
            let upper = patch_offset(u_span, v_span + 1);
            for u_sample in 0..=samples_per_span {
                let lower_index = lower + samples_per_span * side + u_sample;
                let upper_index = upper + u_sample;
                snap_first_point_to_second_if_near(vertices, lower_index, upper_index, tolerance)?;
            }
        }
    }
    Ok(())
}

fn snap_first_point_to_second_if_near(
    vertices: &mut [Point3],
    first_index: usize,
    second_index: usize,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    let first = vertices[first_index];
    let second = vertices[second_index];
    let scale = first
        .to_array()
        .into_iter()
        .chain(second.to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    let allowed = tolerance.absolute().max(tolerance.relative() * scale);
    if first.distance_to(second)? <= allowed {
        vertices[first_index] = second;
    }
    Ok(())
}

fn push_tessellation_cell(
    vertices: &[Point3],
    faces: &mut Vec<MeshFace>,
    [lower_left, lower_right, upper_right, upper_left]: [u32; 4],
    preserve_quads: bool,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    let first = [lower_left, lower_right, upper_right];
    let second = [lower_left, upper_right, upper_left];
    let first_is_valid = tessellation_triangle_is_nondegenerate(vertices, first, tolerance)?;
    let second_is_valid = tessellation_triangle_is_nondegenerate(vertices, second, tolerance)?;
    if preserve_quads
        && first_is_valid
        && second_is_valid
        && vertices[lower_right as usize] != vertices[upper_left as usize]
    {
        faces.push(MeshFace::Quad([
            lower_left,
            lower_right,
            upper_right,
            upper_left,
        ]));
    } else {
        if first_is_valid {
            faces.push(MeshFace::Triangle(first));
        }
        if second_is_valid {
            faces.push(MeshFace::Triangle(second));
        }
    }
    Ok(())
}

fn tessellation_triangle_is_nondegenerate(
    vertices: &[Point3],
    triangle: [u32; 3],
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let points = triangle.map(|index| vertices[index as usize]);
    let Ok(first) = points[0].vector_to(points[1])?.normalized(tolerance) else {
        return Ok(false);
    };
    let Ok(second) = points[0].vector_to(points[2])?.normalized(tolerance) else {
        return Ok(false);
    };
    Ok(first.as_vector().cross(second.as_vector())?.length()? > tolerance.angular())
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
    fn exact_isocurves_match_non_clamped_rational_surface_evaluation() {
        let controls = (0..4)
            .flat_map(|v| {
                (0..4).map(move |u| {
                    WeightedPoint3::try_new(
                        point(u as Real, v as Real, (u * v) as Real * 0.25),
                        1.0 + (u + 2 * v) as Real * 0.125,
                    )
                    .unwrap()
                })
            })
            .collect();
        let knots_u = vec![-2.0, -1.0, 0.0, 0.8, 2.0, 3.0, 4.0];
        let knots_v = vec![-3.0, -1.0, 0.0, 0.6, 2.0, 4.0, 5.0];
        let surface =
            NurbsSurface::try_new_rational(2, 2, 4, 4, controls, knots_u.clone(), knots_v.clone())
                .unwrap();

        let u_curve = surface.isocurve_u(0.73).unwrap();
        assert_eq!(u_curve.degree(), 2);
        assert_eq!(u_curve.knots(), knots_u);
        assert!(u_curve.is_rational());
        for u in [0.0, 0.19, 0.8, 1.37, 2.0] {
            assert_point_near(
                u_curve.evaluate(u).unwrap(),
                surface.evaluate(u, 0.73).unwrap(),
            );
        }
        let non_clamped_boundary = surface.isocurve_u(*surface.domain_v().start()).unwrap();
        assert_ne!(
            non_clamped_boundary.control_points()[0].point(),
            surface.control_point(0, 0).unwrap().point()
        );
        for u in [0.0, 0.8, 2.0] {
            assert_point_near(
                non_clamped_boundary.evaluate(u).unwrap(),
                surface.evaluate(u, *surface.domain_v().start()).unwrap(),
            );
        }

        let v_curve = surface.isocurve_v(1.21).unwrap();
        assert_eq!(v_curve.degree(), 2);
        assert_eq!(v_curve.knots(), knots_v);
        assert!(v_curve.is_rational());
        for v in [0.0, 0.17, 0.6, 1.41, 2.0] {
            assert_point_near(
                v_curve.evaluate(v).unwrap(),
                surface.evaluate(1.21, v).unwrap(),
            );
        }
        assert!(surface.isocurve_u(-0.1).is_err());
        assert!(surface.isocurve_v(2.1).is_err());
    }

    #[test]
    fn natural_boundary_loops_omit_closed_seams_and_singular_sides() {
        let patch = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 3.0, 0.0),
            point(0.0, 3.0, 0.0),
        ])
        .unwrap();
        assert!(!patch.is_closed_u().unwrap());
        assert!(!patch.is_closed_v().unwrap());
        let patch_loops = patch.natural_boundary_curve_loops().unwrap();
        assert_eq!(patch_loops.len(), 1);
        assert_eq!(patch_loops[0].len(), 4);
        assert_eq!(patch.natural_edge_curves().unwrap(), patch_loops[0]);
        let expected = [
            (point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0)),
            (point(4.0, 0.0, 0.0), point(4.0, 3.0, 0.0)),
            (point(4.0, 3.0, 0.0), point(0.0, 3.0, 0.0)),
            (point(0.0, 3.0, 0.0), point(0.0, 0.0, 0.0)),
        ];
        for (curve, (start, end)) in patch_loops[0].iter().zip(expected) {
            assert_eq!(curve.evaluate(*curve.domain().start()).unwrap(), start);
            assert_eq!(curve.evaluate(*curve.domain().end()).unwrap(), end);
        }

        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame, 2.0, -1.0, 4.0).unwrap();
        assert!(cylinder.is_closed_u().unwrap());
        assert!(!cylinder.is_closed_v().unwrap());
        let cylinder_loops = cylinder.natural_boundary_curve_loops().unwrap();
        assert_eq!(
            cylinder_loops.iter().map(Vec::len).collect::<Vec<_>>(),
            [1, 1]
        );
        assert!(
            cylinder_loops
                .iter()
                .all(|boundary| boundary[0].is_closed().unwrap())
        );
        assert!(
            cylinder_loops
                .iter()
                .all(|boundary| boundary[0].is_rational())
        );
        let cylinder_edges = cylinder.natural_edge_curves().unwrap();
        assert_eq!(cylinder_edges.len(), 3);
        assert!(cylinder_edges[0].is_closed().unwrap());
        assert!(!cylinder_edges[1].is_closed().unwrap());
        assert!(cylinder_edges[2].is_closed().unwrap());

        let cone = NurbsSurface::try_cone(frame, 2.0, 5.0).unwrap();
        assert_eq!(cone.natural_boundary_curve_loops().unwrap().len(), 1);
        assert_eq!(cone.natural_edge_curves().unwrap().len(), 2);

        let sphere = NurbsSurface::try_sphere(frame, 2.0).unwrap();
        assert!(sphere.is_closed_u().unwrap());
        assert!(sphere.natural_boundary_curve_loops().unwrap().is_empty());
        assert_eq!(sphere.natural_edge_curves().unwrap().len(), 1);

        let torus = NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap();
        assert!(torus.is_closed_u().unwrap());
        assert!(torus.is_closed_v().unwrap());
        assert!(torus.natural_boundary_curve_loops().unwrap().is_empty());
        assert_eq!(torus.natural_edge_curves().unwrap().len(), 2);
    }

    #[test]
    fn wireframe_curves_match_opennurbs_density_and_seam_rules() {
        let surface = NurbsSurface::try_new(
            2,
            1,
            5,
            2,
            (0..2)
                .flat_map(|v| {
                    (0..5).map(move |u| point(u as Real, v as Real * 3.0, ((u % 2) * v) as Real))
                })
                .collect(),
            vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        assert_eq!(surface.wire_parameters_u(-1).unwrap(), [0.0, 3.0]);
        assert_eq!(surface.wire_parameters_u(0).unwrap(), [0.0, 1.0, 2.0, 3.0]);
        assert_eq!(surface.wire_parameters_v(1).unwrap(), [0.0, 0.5, 1.0]);
        for (density, expected) in [(-1, 4), (0, 6), (1, 7), (2, 10), (3, 14)] {
            assert_eq!(surface.wireframe_curves(density).unwrap().len(), expected);
        }
        assert!(matches!(
            surface.wireframe_curves(-2),
            Err(GeometryError::InvalidSurfaceWireDensity(-2))
        ));
        assert!(matches!(
            surface.wireframe_curves(100),
            Err(GeometryError::InvalidSurfaceWireDensity(100))
        ));

        let frame = Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 5.0).unwrap();
        for (density, expected) in [(-1, 3), (0, 6), (1, 7), (2, 11)] {
            assert_eq!(cylinder.wireframe_curves(density).unwrap().len(), expected);
        }
        let sphere = NurbsSurface::try_sphere(frame, 2.0).unwrap();
        assert_eq!(sphere.wireframe_curves(1).unwrap().len(), 5);
        let torus = NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap();
        assert_eq!(torus.wireframe_curves(1).unwrap().len(), 8);
    }

    #[test]
    fn exact_surface_area_handles_rational_primitives_and_large_translations() {
        let tolerance = Tolerance::try_new(1.0e-10, 1.0e-12, 1.0e-10).unwrap();
        let translated_rectangle = NurbsSurface::try_bilinear([
            point(1.0e12, -2.0e12, 3.0e12),
            point(1.0e12 + 4.0, -2.0e12, 3.0e12),
            point(1.0e12 + 4.0, -2.0e12 + 3.0, 3.0e12),
            point(1.0e12, -2.0e12 + 3.0, 3.0e12),
        ])
        .unwrap();
        assert!((translated_rectangle.area(tolerance).unwrap() - 12.0).abs() < 1.0e-11);

        let frame = Frame3::try_from_normal(
            point(8.0, -3.0, 2.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            tolerance,
        )
        .unwrap();
        let radius = 2.5;
        let height = 7.0;
        let cases = [
            (
                NurbsSurface::try_disk(frame, radius).unwrap(),
                std::f64::consts::PI * radius * radius,
            ),
            (
                NurbsSurface::try_sphere(frame, radius).unwrap(),
                4.0 * std::f64::consts::PI * radius * radius,
            ),
            (
                NurbsSurface::try_cylinder(frame, radius, -3.0, 4.0).unwrap(),
                2.0 * std::f64::consts::PI * radius * height,
            ),
            (
                NurbsSurface::try_cone(frame, radius, height).unwrap(),
                std::f64::consts::PI * radius * radius.hypot(height),
            ),
            (
                NurbsSurface::try_torus(frame, 5.0, 1.5).unwrap(),
                4.0 * std::f64::consts::PI.powi(2) * 5.0 * 1.5,
            ),
        ];
        for (surface, expected) in cases {
            let area = surface.area(tolerance).unwrap();
            let relative_error = (area - expected).abs() / expected;
            assert!(
                relative_error < 2.0e-11,
                "area {area}, relative error {relative_error}"
            );
        }
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
    fn exact_sphere_matches_opennurbs_control_net_domains_and_orientation() {
        let center = point(1.0, 2.0, 3.0);
        let frame = Frame3::try_from_directions(
            center,
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let surface = NurbsSurface::try_sphere(frame, 2.5).unwrap();
        let half_pi = std::f64::consts::FRAC_PI_2;
        let pi = std::f64::consts::PI;
        let tau = std::f64::consts::TAU;

        assert_eq!(surface.degree_u(), 2);
        assert_eq!(surface.degree_v(), 2);
        assert_eq!(surface.control_point_count_u(), 9);
        assert_eq!(surface.control_point_count_v(), 5);
        assert_eq!(surface.domain_u(), 0.0..=tau);
        assert_eq!(surface.domain_v(), -half_pi..=half_pi);
        assert_eq!(
            surface.knots_u(),
            &[
                0.0,
                0.0,
                0.0,
                half_pi,
                half_pi,
                pi,
                pi,
                3.0 * half_pi,
                3.0 * half_pi,
                tau,
                tau,
                tau,
            ]
        );
        assert_eq!(
            surface.knots_v(),
            &[
                -half_pi, -half_pi, -half_pi, 0.0, 0.0, half_pi, half_pi, half_pi,
            ]
        );
        assert_eq!(
            surface.control_point(0, 0).unwrap().point(),
            point(1.0, 2.0, 0.5)
        );
        assert_eq!(
            surface.control_point(1, 0).unwrap().weight(),
            0.5_f64.sqrt()
        );
        assert_eq!(
            surface.control_point(0, 1).unwrap().point(),
            point(1.0, 4.5, 0.5)
        );
        assert!(Tolerance::DEFAULT.approx_eq(surface.control_point(1, 1).unwrap().weight(), 0.5));
        assert_eq!(
            surface.control_point(2, 2).unwrap().point(),
            point(-1.5, 2.0, 3.0)
        );
        assert_eq!(
            surface.control_point(8, 4).unwrap().point(),
            point(1.0, 2.0, 5.5)
        );

        assert_point_near(
            surface.evaluate(0.0, -half_pi).unwrap(),
            point(1.0, 2.0, 0.5),
        );
        assert_point_near(surface.evaluate(0.0, 0.0).unwrap(), point(1.0, 4.5, 3.0));
        assert_point_near(
            surface.evaluate(half_pi, 0.0).unwrap(),
            point(-1.5, 2.0, 3.0),
        );
        for u_index in 0..32 {
            for v_index in 0..=16 {
                let u = tau * u_index as Real / 32.0;
                let v = -half_pi + pi * v_index as Real / 16.0;
                let radius = surface.evaluate(u, v).unwrap().distance_to(center).unwrap();
                assert!(Tolerance::DEFAULT.approx_eq(radius, 2.5));
            }
        }
        let display_mesh = surface.tessellate(2, Tolerance::DEFAULT).unwrap();
        assert!(!display_mesh.triangles().is_empty());
        assert!(NurbsSurface::try_sphere(frame, 0.0).is_err());
        assert!(NurbsSurface::try_sphere(frame, -1.0).is_err());
        assert!(NurbsSurface::try_sphere(frame, Real::INFINITY).is_err());
    }

    #[test]
    fn exact_ellipsoid_is_an_affine_sphere_with_preserved_parameterization() {
        let center = point(1.0, 2.0, 3.0);
        let frame = Frame3::try_from_directions(
            center,
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let radii = [2.0, 3.0, 4.0];
        let surface = NurbsSurface::try_ellipsoid(frame, radii).unwrap();
        let half_pi = std::f64::consts::FRAC_PI_2;
        let tau = std::f64::consts::TAU;

        assert_eq!(surface.degree_u(), 2);
        assert_eq!(surface.degree_v(), 2);
        assert_eq!(surface.control_point_count_u(), 9);
        assert_eq!(surface.control_point_count_v(), 5);
        assert_eq!(surface.domain_u(), 0.0..=tau);
        assert_eq!(surface.domain_v(), -half_pi..=half_pi);
        assert_eq!(
            surface.control_point(0, 0).unwrap().point(),
            point(1.0, 2.0, -1.0)
        );
        assert_eq!(
            surface.control_point(0, 1).unwrap().point(),
            point(1.0, 4.0, -1.0)
        );
        assert_eq!(
            surface.control_point(1, 1).unwrap().point(),
            point(-2.0, 4.0, -1.0)
        );
        assert!(Tolerance::DEFAULT.approx_eq(surface.control_point(1, 1).unwrap().weight(), 0.5));
        assert_eq!(
            surface.control_point(8, 4).unwrap().point(),
            point(1.0, 2.0, 7.0)
        );
        assert_point_near(
            surface.evaluate(0.0, -half_pi).unwrap(),
            point(1.0, 2.0, -1.0),
        );
        assert_point_near(surface.evaluate(0.0, 0.0).unwrap(), point(1.0, 4.0, 3.0));
        assert_point_near(
            surface.evaluate(half_pi, 0.0).unwrap(),
            point(-2.0, 2.0, 3.0),
        );
        assert_point_near(
            surface.evaluate(0.0, half_pi).unwrap(),
            point(1.0, 2.0, 7.0),
        );

        for u_index in 0..32 {
            for v_index in 0..=16 {
                let u = tau * u_index as Real / 32.0;
                let v = -half_pi + std::f64::consts::PI * v_index as Real / 16.0;
                let offset = center.vector_to(surface.evaluate(u, v).unwrap()).unwrap();
                let normalized = frame
                    .axes()
                    .into_iter()
                    .zip(radii)
                    .map(|(axis, radius)| offset.dot(axis.as_vector()).unwrap() / radius)
                    .map(|coordinate| coordinate * coordinate)
                    .sum::<Real>();
                assert!(Tolerance::DEFAULT.approx_eq(normalized, 1.0));
            }
        }
        assert!(
            !surface
                .tessellate(2, Tolerance::DEFAULT)
                .unwrap()
                .triangles()
                .is_empty()
        );
        assert!(NurbsSurface::try_ellipsoid(frame, [0.0, 1.0, 1.0]).is_err());
        assert!(NurbsSurface::try_ellipsoid(frame, [1.0, -1.0, 1.0]).is_err());
        assert!(NurbsSurface::try_ellipsoid(frame, [1.0, 1.0, Real::NAN]).is_err());
    }

    #[test]
    fn exact_polar_disk_has_a_collapsed_center_and_opposite_frame_normal() {
        let origin = point(1.0, 2.0, 3.0);
        let frame = Frame3::try_from_directions(
            origin,
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let disk = NurbsSurface::try_disk(frame, 4.0).unwrap();

        assert_eq!(disk.degree_u(), 2);
        assert_eq!(disk.degree_v(), 1);
        assert_eq!(disk.control_point_count_u(), 9);
        assert_eq!(disk.control_point_count_v(), 2);
        assert_eq!(disk.domain_u(), 0.0..=std::f64::consts::TAU);
        assert_eq!(disk.domain_v(), 0.0..=4.0);
        for u_index in 0..=16 {
            let u = std::f64::consts::TAU * u_index as Real / 16.0;
            assert!(
                disk.evaluate(u, 0.0)
                    .unwrap()
                    .is_near(origin, Tolerance::DEFAULT)
            );
            assert!(Tolerance::DEFAULT.approx_eq(
                disk.evaluate(u, 4.0).unwrap().distance_to(origin).unwrap(),
                4.0
            ));
        }
        assert_eq!(disk.evaluate(0.0, 2.0).unwrap(), point(1.0, 4.0, 3.0));
        let normal = disk.normal_at(0.25, 2.0, Tolerance::DEFAULT).unwrap();
        assert!(normal.as_vector().dot(frame.z_axis().as_vector()).unwrap() < -0.999_999);
        assert!(
            !disk
                .tessellate(2, Tolerance::DEFAULT)
                .unwrap()
                .triangles()
                .is_empty()
        );
        assert!(NurbsSurface::try_disk(frame, 0.0).is_err());
        assert!(NurbsSurface::try_disk(frame, Real::NAN).is_err());
    }

    #[test]
    fn tessellation_stitches_continuous_full_knot_boundaries_but_preserves_jumps() {
        let tessellate = |right_start: Real| {
            NurbsSurface::try_new(
                1,
                1,
                4,
                2,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(1.0, 0.0, 0.0),
                    point(right_start, 0.0, 0.0),
                    point(right_start + 1.0, 0.0, 0.0),
                    point(0.0, 1.0, 0.0),
                    point(1.0, 1.0, 0.0),
                    point(right_start, 1.0, 0.0),
                    point(right_start + 1.0, 1.0, 0.0),
                ],
                vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0],
                vec![0.0, 0.0, 1.0, 1.0],
            )
            .unwrap()
            .tessellate(1, Tolerance::DEFAULT)
            .unwrap()
        };

        let continuous = tessellate(1.0).topology();
        assert_eq!(continuous.topological_vertex_count(), 6);
        assert_eq!(continuous.boundary_edge_count(), 6);

        let discontinuous = tessellate(3.0).topology();
        assert_eq!(discontinuous.topological_vertex_count(), 8);
        assert_eq!(discontinuous.boundary_edge_count(), 8);
    }

    #[test]
    fn exact_cylinder_matches_opennurbs_signed_height_layout() {
        let center = point(1.0, 2.0, 3.0);
        let frame = Frame3::try_from_directions(
            center,
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let surface = NurbsSurface::try_cylinder(frame, 2.5, 0.0, -4.0).unwrap();

        assert_eq!(surface.degree_u(), 2);
        assert_eq!(surface.degree_v(), 1);
        assert_eq!(surface.control_point_count_u(), 9);
        assert_eq!(surface.control_point_count_v(), 2);
        assert_eq!(surface.domain_u(), 0.0..=std::f64::consts::TAU);
        assert_eq!(surface.domain_v(), -4.0..=0.0);
        assert_eq!(surface.knots_v(), &[-4.0, -4.0, 0.0, 0.0]);
        assert_eq!(
            surface.control_point(0, 0).unwrap().point(),
            point(1.0, 4.5, -1.0)
        );
        assert_eq!(
            surface.control_point(1, 0).unwrap().point(),
            point(-1.5, 4.5, -1.0)
        );
        assert_eq!(
            surface.control_point(0, 1).unwrap().point(),
            point(1.0, 4.5, 3.0)
        );
        assert_eq!(
            surface.control_point(1, 1).unwrap().weight(),
            std::f64::consts::FRAC_1_SQRT_2
        );

        for u_index in 0..32 {
            for v_index in 0..=8 {
                let u = std::f64::consts::TAU * u_index as Real / 32.0;
                let v = -4.0 + 4.0 * v_index as Real / 8.0;
                let point = surface.evaluate(u, v).unwrap();
                let axial = center
                    .vector_to(point)
                    .unwrap()
                    .dot(frame.z_axis().as_vector())
                    .unwrap();
                let axis_point = center
                    .translated(frame.z_axis().as_vector().scaled(axial).unwrap())
                    .unwrap();
                assert!(Tolerance::DEFAULT.approx_eq(point.distance_to(axis_point).unwrap(), 2.5));
            }
        }
        let reversed = NurbsSurface::try_cylinder(frame, 2.5, -4.0, 0.0).unwrap();
        assert_eq!(surface, reversed);
        assert!(
            !surface
                .tessellate(2, Tolerance::DEFAULT)
                .unwrap()
                .triangles()
                .is_empty()
        );
        assert!(NurbsSurface::try_cylinder(frame, 0.0, 0.0, 1.0).is_err());
        assert!(NurbsSurface::try_cylinder(frame, 1.0, 2.0, 2.0).is_err());
        assert!(NurbsSurface::try_cylinder(frame, 1.0, 0.0, Real::NAN).is_err());
    }

    #[test]
    fn exact_truncated_cone_uses_rhino_slant_length_parameterization() {
        let center = point(1.0, 2.0, 3.0);
        let frame = Frame3::try_from_directions(
            center,
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let surface = NurbsSurface::try_truncated_cone(frame, [3.0, 1.0], 4.0).unwrap();
        let slant_length = 20.0_f64.sqrt();

        assert_eq!(surface.degree_u(), 2);
        assert_eq!(surface.degree_v(), 1);
        assert_eq!(surface.control_point_count_u(), 9);
        assert_eq!(surface.control_point_count_v(), 2);
        assert_eq!(surface.domain_u(), 0.0..=std::f64::consts::TAU);
        assert_eq!(surface.domain_v(), 0.0..=slant_length);
        assert_eq!(surface.knots_v(), &[0.0, 0.0, slant_length, slant_length]);
        assert_eq!(
            surface.control_point(0, 0).unwrap().point(),
            point(1.0, 5.0, 3.0)
        );
        assert_eq!(
            surface.control_point(1, 0).unwrap().point(),
            point(-2.0, 5.0, 3.0)
        );
        assert_eq!(
            surface.control_point(0, 1).unwrap().point(),
            point(1.0, 3.0, 7.0)
        );
        assert_eq!(
            surface.control_point(1, 0).unwrap().weight(),
            std::f64::consts::FRAC_1_SQRT_2
        );
        assert_eq!(
            surface.evaluate(0.0, 0.5 * slant_length).unwrap(),
            point(1.0, 4.0, 5.0)
        );
        let (_, _, derivative_v) = surface
            .evaluate_with_derivatives(0.0, 0.5 * slant_length)
            .unwrap();
        assert!(derivative_v.x().abs() < 1.0e-15);
        assert!((derivative_v.y() + 2.0 / slant_length).abs() < 1.0e-15);
        assert!((derivative_v.z() - 4.0 / slant_length).abs() < 1.0e-15);

        for (radii, height) in [([0.0, 1.0], 4.0), ([3.0, -1.0], 4.0), ([3.0, 1.0], 0.0)] {
            assert!(NurbsSurface::try_truncated_cone(frame, radii, height).is_err());
        }
        assert!(NurbsSurface::try_truncated_cone(frame, [3.0, Real::INFINITY], 4.0).is_err());
    }

    #[test]
    fn exact_cone_matches_opennurbs_apex_and_signed_height_layout() {
        let apex = point(1.0, 2.0, 3.0);
        let frame = Frame3::try_from_directions(
            apex,
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let surface = NurbsSurface::try_cone(frame, 2.5, -4.0).unwrap();

        assert_eq!(surface.degree_u(), 2);
        assert_eq!(surface.degree_v(), 1);
        assert_eq!(surface.control_point_count_u(), 9);
        assert_eq!(surface.control_point_count_v(), 2);
        assert_eq!(surface.domain_u(), 0.0..=std::f64::consts::TAU);
        assert_eq!(surface.domain_v(), -4.0..=0.0);
        assert_eq!(
            surface.control_point(0, 0).unwrap().point(),
            point(1.0, 4.5, -1.0)
        );
        assert_eq!(
            surface.control_point(1, 0).unwrap().point(),
            point(-1.5, 4.5, -1.0)
        );
        for u in 0..9 {
            assert_eq!(surface.control_point(u, 1).unwrap().point(), apex);
            assert_eq!(
                surface.control_point(u, 0).unwrap().weight(),
                surface.control_point(u, 1).unwrap().weight()
            );
        }

        for u_index in 0..32 {
            for v_index in 0..=8 {
                let u = std::f64::consts::TAU * u_index as Real / 32.0;
                let v = -4.0 + 4.0 * v_index as Real / 8.0;
                let point = surface.evaluate(u, v).unwrap();
                let axis_point = apex
                    .translated(frame.z_axis().as_vector().scaled(v).unwrap())
                    .unwrap();
                let expected_radius = 2.5 * (-v / 4.0);
                assert!(
                    Tolerance::DEFAULT
                        .approx_eq(point.distance_to(axis_point).unwrap(), expected_radius)
                );
            }
        }
        assert!(
            !surface
                .tessellate(2, Tolerance::DEFAULT)
                .unwrap()
                .triangles()
                .is_empty()
        );
        let positive = NurbsSurface::try_cone(frame, 2.5, 4.0).unwrap();
        assert_eq!(positive.domain_v(), 0.0..=4.0);
        assert_eq!(positive.control_point(0, 0).unwrap().point(), apex);
        assert_eq!(
            positive.control_point(0, 1).unwrap().point(),
            point(1.0, 4.5, 7.0)
        );
        assert!(NurbsSurface::try_cone(frame, 0.0, 1.0).is_err());
        assert!(NurbsSurface::try_cone(frame, 1.0, 0.0).is_err());
        assert!(NurbsSurface::try_cone(frame, 1.0, Real::INFINITY).is_err());
    }

    #[test]
    fn exact_paraboloid_matches_rhino_meridian_domain_and_tensor_layout() {
        let vertex = point(1.0, 2.0, 3.0);
        let frame = Frame3::try_from_directions(
            vertex,
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let surface = NurbsSurface::try_paraboloid(frame, 2.0, 1.0).unwrap();
        let meridian_length = 2.0_f64.sqrt() + 1.0_f64.asinh();

        assert_eq!(surface.degree_u(), 2);
        assert_eq!(surface.degree_v(), 2);
        assert_eq!(surface.control_point_count_u(), 9);
        assert_eq!(surface.control_point_count_v(), 3);
        assert_eq!(surface.domain_u(), 0.0..=std::f64::consts::TAU);
        assert_eq!(surface.domain_v(), 0.0..=meridian_length);
        assert_eq!(surface.control_point(0, 0).unwrap().point(), vertex);
        assert_eq!(
            surface.control_point(0, 1).unwrap().point(),
            point(1.0, 3.0, 3.0)
        );
        assert_eq!(
            surface.control_point(1, 1).unwrap().point(),
            point(0.0, 3.0, 3.0)
        );
        assert_eq!(
            surface.control_point(0, 2).unwrap().point(),
            point(1.0, 4.0, 4.0)
        );
        assert_eq!(
            surface.control_point(1, 1).unwrap().weight(),
            std::f64::consts::FRAC_1_SQRT_2
        );

        for index in 0..=8 {
            let fraction = index as Real / 8.0;
            let evaluated = surface.evaluate(0.0, meridian_length * fraction).unwrap();
            let expected = point(1.0, 2.0 + 2.0 * fraction, 3.0 + fraction * fraction);
            assert!(
                evaluated.distance_to(expected).unwrap() < 2.0e-15,
                "fraction {fraction}: {evaluated:?} != {expected:?}"
            );
        }

        assert!(NurbsSurface::try_paraboloid(frame, 0.0, 1.0).is_err());
        assert!(NurbsSurface::try_paraboloid(frame, 1.0, 0.0).is_err());
        assert!(NurbsSurface::try_paraboloid(frame, 1.0, Real::INFINITY).is_err());
    }

    #[test]
    fn exact_torus_matches_opennurbs_tensor_domains_and_weights() {
        let center = point(1.0, 2.0, 3.0);
        let frame = Frame3::try_from_directions(
            center,
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let major_radius = 5.0;
        let minor_radius = 1.5;
        let domain_u = std::f64::consts::TAU * major_radius;
        let domain_v = std::f64::consts::TAU * minor_radius;
        let surface = NurbsSurface::try_torus(frame, major_radius, minor_radius).unwrap();

        assert_eq!(surface.degree_u(), 2);
        assert_eq!(surface.degree_v(), 2);
        assert_eq!(surface.control_point_count_u(), 9);
        assert_eq!(surface.control_point_count_v(), 9);
        assert_eq!(surface.domain_u(), 0.0..=domain_u);
        assert_eq!(surface.domain_v(), 0.0..=domain_v);
        assert_eq!(surface.knots_u()[3], domain_u * 0.25);
        assert_eq!(surface.knots_u()[7], domain_u * 0.75);
        assert_eq!(surface.knots_v()[3], domain_v * 0.25);
        assert_eq!(surface.knots_v()[7], domain_v * 0.75);
        assert_eq!(
            surface.control_point(0, 0).unwrap().point(),
            point(1.0, 8.5, 3.0)
        );
        assert_eq!(
            surface.control_point(1, 0).unwrap().point(),
            point(-5.5, 8.5, 3.0)
        );
        assert_eq!(
            surface.control_point(0, 1).unwrap().point(),
            point(1.0, 8.5, 4.5)
        );
        assert_eq!(
            surface.control_point(0, 2).unwrap().point(),
            point(1.0, 7.0, 4.5)
        );
        assert_eq!(
            surface.control_point(0, 4).unwrap().point(),
            point(1.0, 5.5, 3.0)
        );
        assert_eq!(
            surface.control_point(1, 0).unwrap().weight(),
            std::f64::consts::FRAC_1_SQRT_2
        );
        assert_eq!(
            surface.control_point(0, 1).unwrap().weight(),
            std::f64::consts::FRAC_1_SQRT_2
        );
        assert!(Tolerance::DEFAULT.approx_eq(surface.control_point(1, 1).unwrap().weight(), 0.5));

        assert_point_near(surface.evaluate(0.0, 0.0).unwrap(), point(1.0, 8.5, 3.0));
        assert_point_near(
            surface.evaluate(domain_u * 0.25, 0.0).unwrap(),
            point(-5.5, 2.0, 3.0),
        );
        assert_point_near(
            surface.evaluate(0.0, domain_v * 0.25).unwrap(),
            point(1.0, 7.0, 4.5),
        );
        for u_index in 0..32 {
            for v_index in 0..32 {
                let u = domain_u * u_index as Real / 32.0;
                let v = domain_v * v_index as Real / 32.0;
                let point = surface.evaluate(u, v).unwrap();
                let offset = center.vector_to(point).unwrap();
                let axial = offset.dot(frame.z_axis().as_vector()).unwrap();
                let axial_vector = frame.z_axis().as_vector().scaled(axial).unwrap();
                let radial = Vector3::try_new(
                    offset.x() - axial_vector.x(),
                    offset.y() - axial_vector.y(),
                    offset.z() - axial_vector.z(),
                )
                .unwrap()
                .length()
                .unwrap();
                assert!(
                    Tolerance::DEFAULT
                        .approx_eq((radial - major_radius).hypot(axial), minor_radius)
                );
            }
        }
        assert!(
            !surface
                .tessellate(2, Tolerance::DEFAULT)
                .unwrap()
                .triangles()
                .is_empty()
        );
        assert!(NurbsSurface::try_torus(frame, 1.0, 0.0).is_err());
        assert!(NurbsSurface::try_torus(frame, 1.0, 1.0).is_err());
        assert!(NurbsSurface::try_torus(frame, 1.0, 2.0).is_err());
        assert!(NurbsSurface::try_torus(frame, Real::INFINITY, 1.0).is_err());
        assert!(NurbsSurface::try_torus(frame, Real::MAX, 1.0).is_err());
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
    fn exact_curve_along_curve_extrusion_is_a_rational_sum_surface() {
        let profile = crate::NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(4.0, 0.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(4.0, 1.0, 0.0), 0.75).unwrap(),
                WeightedPoint3::try_new(point(3.0, 1.0, 0.0), 1.0).unwrap(),
            ],
            vec![3.0, 3.0, 3.0, 8.0, 8.0, 8.0],
        )
        .unwrap();
        let path = crate::NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(10.0, 0.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(11.0, 2.0, 1.0), 0.5).unwrap(),
                WeightedPoint3::try_new(point(12.0, 3.0, 4.0), 1.0).unwrap(),
            ],
            vec![2.0, 2.0, 2.0, 7.0, 7.0, 7.0],
        )
        .unwrap();
        let surface = NurbsSurface::try_extruded_curve_along_curve(&profile, &path).unwrap();

        assert_eq!(surface.degree_u(), profile.degree());
        assert_eq!(surface.degree_v(), path.degree());
        assert_eq!(surface.control_point_count_u(), 3);
        assert_eq!(surface.control_point_count_v(), 3);
        assert_eq!(surface.knots_u(), profile.knots());
        assert_eq!(surface.knots_v(), path.knots());
        assert!(surface.is_rational());
        assert_eq!(
            surface.control_point(0, 0),
            profile.control_points().first().copied()
        );
        assert_eq!(
            surface.control_point(1, 1).unwrap().point(),
            point(5.0, 3.0, 1.0)
        );
        assert_eq!(surface.control_point(1, 1).unwrap().weight(), 0.375);
        assert_eq!(
            surface.control_point(2, 2).unwrap().point(),
            point(5.0, 4.0, 4.0)
        );
        assert_eq!(surface.control_point(2, 2).unwrap().weight(), 1.0);

        let path_start = path.evaluate(2.0).unwrap();
        for (u, v) in [(3.0, 2.0), (5.5, 4.25), (8.0, 7.0)] {
            let profile_point = profile.evaluate(u).unwrap();
            let expected = profile_point
                .translated(path_start.vector_to(path.evaluate(v).unwrap()).unwrap())
                .unwrap();
            assert_point_near(surface.evaluate(u, v).unwrap(), expected);
        }
        let (_, derivative_u, derivative_v) = surface.evaluate_with_derivatives(5.5, 4.25).unwrap();
        for (actual, expected) in derivative_u
            .to_array()
            .into_iter()
            .zip(profile.derivative_at(5.5).unwrap().to_array())
        {
            assert!(Tolerance::DEFAULT.approx_eq(actual, expected));
        }
        for (actual, expected) in derivative_v
            .to_array()
            .into_iter()
            .zip(path.derivative_at(4.25).unwrap().to_array())
        {
            assert!(Tolerance::DEFAULT.approx_eq(actual, expected));
        }

        let constant_path = crate::NurbsCurve::try_new(
            1,
            vec![point(1.0, 2.0, 3.0), point(1.0, 2.0, 3.0)],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        assert!(NurbsSurface::try_extruded_curve_along_curve(&profile, &constant_path).is_err());
    }

    #[test]
    fn exact_curve_to_point_extrusion_matches_rhino_direction_and_weights() {
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
        let apex = point(1.0, 2.0, 5.0);
        let surface = NurbsSurface::try_extruded_curve_to_point(&curve, apex).unwrap();
        let apex_distance = curve.evaluate(2.0).unwrap().distance_to(apex).unwrap();

        assert_eq!(surface.degree_u(), 1);
        assert_eq!(surface.degree_v(), 2);
        assert_eq!(surface.control_point_count_u(), 2);
        assert_eq!(surface.control_point_count_v(), 3);
        assert_eq!(surface.knots_u(), &[0.0, 0.0, apex_distance, apex_distance]);
        assert_eq!(surface.knots_v(), curve.knots());
        assert!(surface.is_rational());
        for (v_index, curve_control) in curve.control_points().iter().enumerate() {
            assert_eq!(surface.control_point(0, v_index), Some(*curve_control));
            let apex_control = surface.control_point(1, v_index).unwrap();
            assert_eq!(apex_control.point(), apex);
            assert_eq!(apex_control.weight(), curve_control.weight());
        }
        for v in [2.0, 3.25, 7.0] {
            assert_point_near(
                surface.evaluate(0.0, v).unwrap(),
                curve.evaluate(v).unwrap(),
            );
            assert_point_near(surface.evaluate(apex_distance, v).unwrap(), apex);
        }
        let (midpoint, derivative_u, _) = surface
            .evaluate_with_derivatives(apex_distance * 0.5, 4.0)
            .unwrap();
        let curve_point = curve.evaluate(4.0).unwrap();
        assert_point_near(
            midpoint,
            curve_point
                .translated(curve_point.vector_to(apex).unwrap().scaled(0.5).unwrap())
                .unwrap(),
        );
        assert_point_near(
            curve_point
                .translated(derivative_u.scaled(apex_distance).unwrap())
                .unwrap(),
            apex,
        );

        assert!(
            NurbsSurface::try_extruded_curve_to_point(&curve, curve.evaluate(2.0).unwrap())
                .is_err()
        );
    }

    #[test]
    fn exact_curve_revolution_matches_rhino_spans_domains_and_tensor_weights() {
        let axis_origin = point(0.0, 0.0, 0.0);
        let axis = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
        let profile = crate::NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(2.0, 0.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(2.0, 0.0, 1.5), 0.75).unwrap(),
                WeightedPoint3::try_new(point(2.0, 0.0, 3.0), 1.0).unwrap(),
            ],
            vec![2.0, 2.0, 2.0, 7.0, 7.0, 7.0],
        )
        .unwrap();
        let full = NurbsSurface::try_revolved_curve(
            &profile,
            axis_origin,
            axis,
            0.0,
            std::f64::consts::TAU,
        )
        .unwrap();
        let quadrant = std::f64::consts::PI;
        let domain_end = 4.0 * std::f64::consts::PI;

        assert_eq!(full.degree_u(), 2);
        assert_eq!(full.degree_v(), profile.degree());
        assert_eq!(full.control_point_count_u(), 9);
        assert_eq!(full.control_point_count_v(), 3);
        assert_eq!(
            full.knots_u(),
            &[
                0.0,
                0.0,
                0.0,
                quadrant,
                quadrant,
                2.0 * quadrant,
                2.0 * quadrant,
                3.0 * quadrant,
                3.0 * quadrant,
                domain_end,
                domain_end,
                domain_end,
            ]
        );
        assert_eq!(full.knots_v(), profile.knots());
        assert_eq!(
            full.control_point(0, 0).unwrap().point(),
            point(2.0, 0.0, 0.0)
        );
        assert_point_near(
            full.control_point(1, 0).unwrap().point(),
            point(2.0, 2.0, 0.0),
        );
        assert_eq!(full.control_point(1, 0).unwrap().weight(), 0.5_f64.sqrt());
        assert_eq!(full.control_point(8, 0), full.control_point(0, 0));
        assert_eq!(
            full.control_point(1, 1).unwrap().weight(),
            0.75 * 0.5_f64.sqrt()
        );
        assert_point_near(full.evaluate(quadrant, 2.0).unwrap(), point(0.0, 2.0, 0.0));
        assert_point_near(
            full.evaluate(2.0 * quadrant, 7.0).unwrap(),
            point(-2.0, 0.0, 3.0),
        );

        let line = crate::NurbsCurve::try_new(
            1,
            vec![point(2.0, 0.0, 0.0), point(2.0, 0.0, 3.0)],
            vec![0.0, 0.0, 3.0, 3.0],
        )
        .unwrap();
        let partial = NurbsSurface::try_revolved_curve(
            &line,
            axis_origin,
            axis,
            30.0_f64.to_radians(),
            120.0_f64.to_radians(),
        )
        .unwrap();
        let partial_end = 2.0 * 120.0_f64.to_radians();
        assert_eq!(partial.control_point_count_u(), 5);
        assert_eq!(
            partial.knots_u(),
            &[
                0.0,
                0.0,
                0.0,
                partial_end * 0.5,
                partial_end * 0.5,
                partial_end,
                partial_end,
                partial_end,
            ]
        );
        assert_point_near(
            partial.evaluate(0.0, 0.0).unwrap(),
            point(3.0_f64.sqrt(), 1.0, 0.0),
        );
        assert_point_near(
            partial.evaluate(partial_end * 0.5, 0.0).unwrap(),
            point(0.0, 2.0, 0.0),
        );
        assert_point_near(
            partial.evaluate(partial_end, 3.0).unwrap(),
            point(-3.0_f64.sqrt(), 1.0, 3.0),
        );
        assert!(Tolerance::DEFAULT.approx_eq(
            partial.control_point(1, 0).unwrap().weight(),
            30.0_f64.to_radians().cos()
        ));

        let negative =
            NurbsSurface::try_revolved_curve(&line, axis_origin, axis, 0.0, -90.0_f64.to_radians())
                .unwrap();
        assert_point_near(
            negative.evaluate(*negative.domain_u().end(), 3.0).unwrap(),
            point(0.0, -2.0, 3.0),
        );
        let wide =
            NurbsSurface::try_revolved_curve(&line, axis_origin, axis, 0.0, 270.0_f64.to_radians())
                .unwrap();
        assert_eq!(wide.control_point_count_u(), 9);

        let x_axis = UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap();
        let x_profile = crate::NurbsCurve::try_new(
            1,
            vec![point(0.0, 2.0, 0.0), point(3.0, 2.0, 0.0)],
            vec![0.0, 0.0, 3.0, 3.0],
        )
        .unwrap();
        let about_x = NurbsSurface::try_revolved_curve(
            &x_profile,
            axis_origin,
            x_axis,
            0.0,
            90.0_f64.to_radians(),
        )
        .unwrap();
        assert_point_near(
            about_x.evaluate(*about_x.domain_u().end(), 0.0).unwrap(),
            point(0.0, 0.0, 2.0),
        );
        assert_point_near(
            about_x.evaluate(*about_x.domain_u().end(), 3.0).unwrap(),
            point(3.0, 0.0, 2.0),
        );

        let axis_touching = crate::NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0, 0.0), point(2.0, 0.0, 3.0)],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        let axis_touching = NurbsSurface::try_revolved_curve(
            &axis_touching,
            axis_origin,
            axis,
            0.0,
            std::f64::consts::TAU,
        )
        .unwrap();
        assert_eq!(axis_touching.domain_u(), 0.0..=domain_end);

        let bulging = crate::NurbsCurve::try_new(
            2,
            vec![
                point(2.0, 0.0, 0.0),
                point(10.0, 0.0, 1.5),
                point(2.0, 0.0, 3.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let bulging = NurbsSurface::try_revolved_curve(
            &bulging,
            axis_origin,
            axis,
            0.0,
            std::f64::consts::TAU,
        )
        .unwrap();
        assert_eq!(bulging.domain_u(), 0.0..=12.0 * std::f64::consts::PI);

        let on_axis = crate::NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0, 0.0), point(0.0, 0.0, 3.0)],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        assert!(
            NurbsSurface::try_revolved_curve(
                &on_axis,
                axis_origin,
                axis,
                0.0,
                std::f64::consts::TAU,
            )
            .is_err()
        );
        assert!(
            NurbsSurface::try_revolved_curve(
                &line,
                axis_origin,
                axis,
                0.0,
                std::f64::consts::TAU + 0.1,
            )
            .is_err()
        );
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
    fn make_uniform_changes_only_requested_surface_knot_directions() {
        let controls = (0..5)
            .flat_map(|v| {
                (0..4).map(move |u| {
                    WeightedPoint3::try_new(
                        point(u as Real, v as Real, (u * v) as Real * 0.1),
                        0.5 + (u + 2 * v) as Real * 0.1,
                    )
                    .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let surface = NurbsSurface::try_new_rational(
            2,
            3,
            4,
            5,
            controls.clone(),
            vec![0.0, 0.0, 0.0, 0.25, 1.0, 1.0, 1.0],
            vec![10.0, 10.0, 10.0, 10.0, 13.0, 20.0, 20.0, 20.0, 20.0],
        )
        .unwrap();

        let uniform_u = surface.try_make_uniform(SurfaceKnotDirection::U).unwrap();
        assert_eq!(uniform_u.knots_u(), &[0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]);
        assert_eq!(uniform_u.knots_v(), surface.knots_v());
        assert_eq!(uniform_u.control_points(), controls);

        let uniform_v = surface.try_make_uniform(SurfaceKnotDirection::V).unwrap();
        assert_eq!(uniform_v.knots_u(), surface.knots_u());
        assert_eq!(
            uniform_v.knots_v(),
            &[0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0]
        );

        let both = surface
            .try_make_uniform(SurfaceKnotDirection::Both)
            .unwrap();
        assert_eq!(both.knots_u(), uniform_u.knots_u());
        assert_eq!(both.knots_v(), uniform_v.knots_v());
        assert_eq!(both.domain_u(), 0.0..=2.0);
        assert_eq!(both.domain_v(), 0.0..=2.0);
        assert!(both.is_rational());
    }

    #[test]
    fn change_degree_elevates_surfaces_exactly_and_supports_signed_weights() {
        let row = [
            ([-1.0, 0.0, 0.0], 0.75),
            ([2.0, -2.0, 0.0], 1.5),
            ([4.0, 2.0, 1.0], 0.6),
            ([0.0, 4.0, 0.0], 1.8),
            ([-1.0, 0.0, 0.0], 0.75),
        ];
        let controls = row
            .into_iter()
            .chain(row.into_iter().map(|(mut point, weight)| {
                point[0] += 0.5;
                point[2] += 4.0;
                (point, weight * 1.2)
            }))
            .map(|(point, weight)| {
                WeightedPoint3::try_new(Point3::try_from(point).unwrap(), weight).unwrap()
            })
            .collect();
        let source = NurbsSurface::try_new_rational(
            2,
            1,
            5,
            2,
            controls,
            vec![0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 8.0, 8.0],
            vec![-3.0, -3.0, 2.0, 2.0],
        )
        .unwrap();

        let elevated = source.try_change_degree(4, 3, false).unwrap();
        assert_eq!((elevated.degree_u(), elevated.degree_v()), (4, 3));
        assert_eq!(
            (
                elevated.control_point_count_u(),
                elevated.control_point_count_v()
            ),
            (11, 4)
        );
        assert_eq!(
            elevated.knots_u(),
            &[
                0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 5.0, 5.0, 5.0, 8.0, 8.0, 8.0, 8.0, 8.0,
            ]
        );
        assert_eq!(
            elevated.knots_v(),
            &[-3.0, -3.0, -3.0, -3.0, 2.0, 2.0, 2.0, 2.0]
        );
        for v_sample in 0..=8 {
            for u_sample in 0..=8 {
                let u = u_sample as Real;
                let v = -3.0 + 5.0 * v_sample as Real / 8.0;
                assert_point_near(
                    elevated.evaluate(u, v).unwrap(),
                    source.evaluate(u, v).unwrap(),
                );
            }
        }

        let deformable = source.try_change_degree(4, 3, true).unwrap();
        assert_eq!(
            (
                deformable.control_point_count_u(),
                deformable.control_point_count_v()
            ),
            (7, 4)
        );
        assert!(
            deformable
                .control_points()
                .iter()
                .any(|control| control.weight() < 0.0)
        );
        assert_eq!(source.try_change_degree(2, 1, true).unwrap(), source);
        assert_eq!(
            source.try_change_degree(0, 1, false),
            Err(GeometryError::InvalidDegree)
        );
    }

    #[test]
    fn make_periodic_maps_closed_surface_rows_and_columns() {
        let row = [
            point(1.0, 0.0, 0.0),
            point(4.0, -1.0, 0.0),
            point(6.0, 2.0, 1.0),
            point(4.0, 5.0, 0.0),
            point(0.0, 4.0, -1.0),
            point(-2.0, 1.0, 0.0),
            point(1.0, 0.0, 0.0),
        ];
        let translation = Vector3::try_new(0.5, 0.0, 3.0).unwrap();
        let translated = row.map(|point| point.translated(translation).unwrap());
        let mut controls = row.to_vec();
        controls.extend(translated);
        let knots = vec![
            10.0, 10.0, 10.0, 10.0, 11.0, 13.0, 19.0, 25.0, 25.0, 25.0, 25.0,
        ];
        let surface = NurbsSurface::try_new(
            3,
            1,
            7,
            2,
            controls,
            knots.clone(),
            vec![2.0, 2.0, 9.0, 9.0],
        )
        .unwrap();

        let periodic_u = surface
            .try_make_periodic(SurfaceKnotDirection::U, false)
            .unwrap();

        assert!(periodic_u.is_periodic_u());
        assert!(!periodic_u.is_periodic_v());
        assert_eq!(periodic_u.control_point_count_u(), 9);
        assert_eq!(periodic_u.control_point_count_v(), 2);
        assert_eq!(periodic_u.knots_v(), surface.knots_v());
        for (v, expected_row) in [row, translated].into_iter().enumerate() {
            let expected = NurbsCurve::try_new(3, expected_row.to_vec(), knots.clone())
                .unwrap()
                .try_make_periodic(false)
                .unwrap();
            for u in 0..expected.control_points().len() {
                assert_eq!(
                    periodic_u.control_point(u, v),
                    Some(expected.control_points()[u])
                );
            }
            assert_eq!(periodic_u.knots_u(), expected.knots());
        }

        let controls = (0..7)
            .flat_map(|v| [row[v], translated[v]])
            .collect::<Vec<_>>();
        let transposed =
            NurbsSurface::try_new(1, 3, 2, 7, controls, vec![2.0, 2.0, 9.0, 9.0], knots).unwrap();
        let periodic_v = transposed
            .try_make_periodic(SurfaceKnotDirection::V, true)
            .unwrap();
        assert!(!periodic_v.is_periodic_u());
        assert!(periodic_v.is_periodic_v());
        assert_eq!(periodic_v.control_point_count_u(), 2);
        assert_eq!(periodic_v.control_point_count_v(), 7);
        assert_eq!(periodic_v.knots_u(), transposed.knots_u());
    }

    #[test]
    fn make_periodic_can_convert_both_closed_surface_directions() {
        let coordinates = [0.0, 2.0, 4.0, 1.0, 0.0];
        let controls = (0..5)
            .flat_map(|v| {
                (0..5).map(move |u| {
                    point(
                        coordinates[u],
                        coordinates[v],
                        coordinates[u] * coordinates[v] * 0.1,
                    )
                })
            })
            .collect::<Vec<_>>();
        let surface = NurbsSurface::try_new(
            2,
            2,
            5,
            5,
            controls,
            vec![0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 8.0, 8.0],
            vec![-2.0, -2.0, -2.0, 1.0, 4.0, 9.0, 9.0, 9.0],
        )
        .unwrap();
        assert!(surface.is_closed_u().unwrap());
        assert!(surface.is_closed_v().unwrap());

        let sharp = surface
            .try_make_periodic(SurfaceKnotDirection::Both, false)
            .unwrap();
        assert!(sharp.is_periodic_u());
        assert!(sharp.is_periodic_v());
        assert_eq!(sharp.control_point_count_u(), 7);
        assert_eq!(sharp.control_point_count_v(), 7);

        let smooth = surface
            .try_make_periodic(SurfaceKnotDirection::Both, true)
            .unwrap();
        assert!(smooth.is_periodic_u());
        assert!(smooth.is_periodic_v());
        assert_eq!(smooth.control_point_count_u(), 5);
        assert_eq!(smooth.control_point_count_v(), 5);
        assert_eq!(smooth.domain_u(), surface.domain_u());
        assert_eq!(smooth.domain_v(), surface.domain_v());
    }

    #[test]
    fn make_periodic_surface_validates_requested_direction() {
        let open = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
            point(0.0, 2.0, 0.0),
        ])
        .unwrap();
        assert_eq!(
            open.try_make_periodic(SurfaceKnotDirection::U, false),
            Err(GeometryError::PeriodicSurfaceDirectionMustBeClosed { direction: "U" })
        );

        let linear_closed = NurbsSurface::try_new(
            1,
            1,
            3,
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 3.0),
                point(2.0, 0.0, 3.0),
                point(0.0, 0.0, 3.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 2.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        assert_eq!(
            linear_closed.try_make_periodic(SurfaceKnotDirection::U, false),
            Err(GeometryError::PeriodicNurbsDegreeTooLow)
        );
    }

    #[test]
    fn knot_insertion_refines_rational_surface_rows_and_columns_exactly() {
        let controls = (0..4)
            .flat_map(|v| {
                (0..4).map(move |u| {
                    WeightedPoint3::try_new(
                        point(u as Real, v as Real, (u * v) as Real * 0.2),
                        0.6 + (2 * u + v) as Real * 0.15,
                    )
                    .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let surface = NurbsSurface::try_new_rational(
            2,
            2,
            4,
            4,
            controls,
            vec![0.0, 0.0, 0.0, 0.35, 1.0, 1.0, 1.0],
            vec![2.0, 2.0, 2.0, 2.6, 4.0, 4.0, 4.0],
        )
        .unwrap();

        let refined_u = surface.try_insert_knot_u(0.52, 2).unwrap();
        assert_eq!(refined_u.control_point_count_u(), 6);
        assert_eq!(refined_u.control_point_count_v(), 4);
        assert_eq!(
            refined_u.knots_u(),
            &[0.0, 0.0, 0.0, 0.35, 0.52, 0.52, 1.0, 1.0, 1.0]
        );
        assert_eq!(refined_u.knots_v(), surface.knots_v());

        let refined_v = surface.try_insert_knot_v(3.1, 1).unwrap();
        assert_eq!(refined_v.control_point_count_u(), 4);
        assert_eq!(refined_v.control_point_count_v(), 5);
        assert_eq!(refined_v.knots_u(), surface.knots_u());
        assert_eq!(
            refined_v.knots_v(),
            &[2.0, 2.0, 2.0, 2.6, 3.1, 4.0, 4.0, 4.0]
        );

        for u_index in 0..=12 {
            let u = u_index as Real / 12.0;
            for v_index in 0..=12 {
                let v = 2.0 + v_index as Real / 6.0;
                let expected = surface.evaluate(u, v).unwrap();
                assert_point_near(refined_u.evaluate(u, v).unwrap(), expected);
                assert_point_near(refined_v.evaluate(u, v).unwrap(), expected);
            }
        }
    }

    #[test]
    fn knot_removal_maps_surface_rows_and_columns_like_rhino() {
        let controls = (0..4)
            .flat_map(|v| {
                (0..5).map(move |u| {
                    let z = [
                        [0.0, 2.0, -1.0, 3.0, 0.0],
                        [1.0, 5.0, 0.0, 4.0, 2.0],
                        [-1.0, 1.0, 4.0, -2.0, 3.0],
                        [2.0, -1.0, 3.0, 1.0, 0.0],
                    ][v][u];
                    point([0.0, 2.0, 5.0, 8.0, 11.0][u], [0.0, 3.0, 7.0, 10.0][v], z)
                })
            })
            .collect();
        let surface = NurbsSurface::try_new(
            2,
            2,
            5,
            4,
            controls,
            vec![0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 8.0, 8.0],
            vec![-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0],
        )
        .unwrap();

        let removed_u = surface.try_remove_knot_u_near(4.8).unwrap();
        assert_eq!(
            (
                removed_u.control_point_count_u(),
                removed_u.control_point_count_v()
            ),
            (4, 4)
        );
        assert_eq!(removed_u.knots_u(), &[0.0, 0.0, 0.0, 2.0, 8.0, 8.0, 8.0]);
        assert_eq!(removed_u.knots_v(), surface.knots_v());
        assert_point_near(
            removed_u.control_point(1, 0).unwrap().point(),
            point(2.0750000000000006, 0.0, 1.6333333333333337),
        );

        let removed_v = surface.try_remove_knot_v_near(1.1).unwrap();
        assert_eq!(
            (
                removed_v.control_point_count_u(),
                removed_v.control_point_count_v()
            ),
            (5, 3)
        );
        assert_eq!(removed_v.knots_u(), surface.knots_u());
        assert_eq!(removed_v.knots_v(), &[-2.0, -2.0, -2.0, 6.0, 6.0, 6.0]);
        assert_point_near(
            removed_v.control_point(0, 1).unwrap().point(),
            point(0.0, 6.040000000000003, -1.1600000000000001),
        );

        let periodic_row = [
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
        ];
        let mut periodic_controls = periodic_row.to_vec();
        periodic_controls.extend(periodic_row.map(|point| {
            point
                .translated(Vector3::try_new(0.0, 0.0, 3.0).unwrap())
                .unwrap()
        }));
        let periodic = NurbsSurface::try_new(
            3,
            1,
            7,
            2,
            periodic_controls,
            vec![0.0, 0.0, 1.0, 3.0, 6.0, 10.0, 11.0, 13.0, 16.0, 20.0, 20.0],
            vec![0.0, 0.0, 5.0, 5.0],
        )
        .unwrap();
        assert!(periodic.is_periodic_u());
        assert_eq!(
            periodic.try_remove_knot_u_near(6.0),
            Err(GeometryError::PeriodicKnotRemovalUnsupported {
                direction: "surface U direction"
            })
        );
    }

    #[test]
    fn control_point_removal_drops_complete_surface_rows() {
        let controls = (0..4)
            .flat_map(|v| {
                (0..5).map(move |u| {
                    point(
                        [0.0, 2.0, 5.0, 8.0, 11.0][u],
                        [0.0, 3.0, 7.0, 10.0][v],
                        (u * v) as Real,
                    )
                })
            })
            .collect();
        let surface = NurbsSurface::try_new(
            2,
            2,
            5,
            4,
            controls,
            vec![0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 8.0, 8.0],
            vec![-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0],
        )
        .unwrap();

        let removed_u = surface
            .try_remove_control_point(SurfaceKnotDirection::U, 2)
            .unwrap();
        assert_eq!(
            (
                removed_u.control_point_count_u(),
                removed_u.control_point_count_v()
            ),
            (4, 4)
        );
        assert_eq!(removed_u.knots_u(), &[0.0, 0.0, 0.0, 5.0, 8.0, 8.0, 8.0]);
        assert_eq!(removed_u.knots_v(), surface.knots_v());
        for v in 0..4 {
            assert_eq!(removed_u.control_point(0, v), surface.control_point(0, v));
            assert_eq!(removed_u.control_point(1, v), surface.control_point(1, v));
            assert_eq!(removed_u.control_point(2, v), surface.control_point(3, v));
            assert_eq!(removed_u.control_point(3, v), surface.control_point(4, v));
        }

        let removed_v = surface
            .try_remove_control_point(SurfaceKnotDirection::V, 1)
            .unwrap();
        assert_eq!(
            (
                removed_v.control_point_count_u(),
                removed_v.control_point_count_v()
            ),
            (5, 3)
        );
        assert_eq!(removed_v.knots_u(), surface.knots_u());
        assert_eq!(removed_v.knots_v(), &[-2.0, -2.0, -2.0, 6.0, 6.0, 6.0]);
        for u in 0..5 {
            assert_eq!(removed_v.control_point(u, 0), surface.control_point(u, 0));
            assert_eq!(removed_v.control_point(u, 1), surface.control_point(u, 2));
            assert_eq!(removed_v.control_point(u, 2), surface.control_point(u, 3));
        }

        assert!(matches!(
            surface.try_remove_control_point(SurfaceKnotDirection::U, 5),
            Err(GeometryError::ControlPointIndexOutOfRange {
                direction: "surface U direction",
                index: 5,
                control_point_count: 5,
            })
        ));
        assert!(
            surface
                .try_remove_control_point(SurfaceKnotDirection::Both, 0)
                .is_err()
        );
    }

    #[test]
    fn multiple_knot_removal_matches_rhino_surface_order_and_crease_samples() {
        let profile = NurbsCurve::try_new(
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
        let surface = NurbsSurface::try_extruded_curve(
            &profile,
            Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 4.0).unwrap(),
        )
        .unwrap();

        let (ordinary, removed) = surface
            .try_remove_multiple_knots(SurfaceKnotDirection::Both, false, 0.0)
            .unwrap();
        assert_eq!(removed, 1);
        assert_eq!(
            (
                ordinary.control_point_count_u(),
                ordinary.control_point_count_v()
            ),
            (9, 2)
        );
        assert_eq!(
            ordinary.knots_u(),
            &[
                0.0, 0.0, 0.0, 0.0, 2.0, 5.0, 5.0, 5.0, 7.0, 10.0, 10.0, 10.0, 10.0
            ]
        );
        assert_point_near(
            ordinary.control_point(1, 0).unwrap().point(),
            point(1.2511378848728234, 1.2599732262382861, 1.627844712182061),
        );
        assert_point_near(
            ordinary.control_point(1, 1).unwrap().point(),
            point(1.2511378848728234, 1.2599732262382861, 5.627844712182061),
        );

        let (below_crease, removed) = surface
            .try_remove_multiple_knots(SurfaceKnotDirection::U, true, 130.0_f64.to_radians())
            .unwrap();
        assert_eq!((below_crease, removed), (ordinary, 1));
        let (all, removed) = surface
            .try_remove_multiple_knots(SurfaceKnotDirection::U, true, 135.0_f64.to_radians())
            .unwrap();
        assert_eq!(removed, 3);
        assert_eq!(
            (all.control_point_count_u(), all.control_point_count_v()),
            (7, 2)
        );
        assert_eq!(
            all.knots_u(),
            &[0.0, 0.0, 0.0, 0.0, 2.0, 5.0, 7.0, 10.0, 10.0, 10.0, 10.0]
        );
        assert_point_near(
            all.control_point(1, 0).unwrap().point(),
            point(1.277439372269475, 0.7925502880770412, 1.80206788629584),
        );

        let varying_crease = NurbsSurface::try_new(
            1,
            1,
            3,
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.984807753012208, 0.17364817766693033, 0.0),
                point(0.0, 0.0, 1.0),
                point(1.0, 0.0, 1.0),
                point(0.8263518223330697, 0.984807753012208, 1.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 2.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        let (unchanged, removed) = varying_crease
            .try_remove_multiple_knots(SurfaceKnotDirection::U, true, 50.0_f64.to_radians())
            .unwrap();
        assert_eq!((unchanged, removed), (varying_crease.clone(), 0));
        let (linearized, removed) = varying_crease
            .try_remove_multiple_knots(SurfaceKnotDirection::U, true, 101.0_f64.to_radians())
            .unwrap();
        assert_eq!(removed, 1);
        assert_eq!(
            (
                linearized.control_point_count_u(),
                linearized.control_point_count_v()
            ),
            (2, 2)
        );
        assert_eq!(linearized.knots_u(), &[0.0, 0.0, 2.0, 2.0]);

        let knots = vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0];
        let tensor = NurbsSurface::try_new(
            3,
            3,
            6,
            6,
            (0..6)
                .flat_map(|v| {
                    (0..6).map(move |u| point(u as Real, v as Real, (u * v) as Real * 0.1))
                })
                .collect(),
            knots.clone(),
            knots,
        )
        .unwrap();
        let (tensor, removed) = tensor
            .try_remove_multiple_knots(SurfaceKnotDirection::Both, false, 0.0)
            .unwrap();
        assert_eq!(removed, 2);
        assert_eq!(
            (
                tensor.control_point_count_u(),
                tensor.control_point_count_v()
            ),
            (5, 5)
        );
    }

    #[test]
    fn make_uniform_retains_periodic_surface_direction() {
        let row = [
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
        ];
        let mut controls = row.to_vec();
        controls.extend(row.into_iter().map(|point| {
            point
                .translated(Vector3::try_new(0.0, 0.0, 3.0).unwrap())
                .unwrap()
        }));
        let surface = NurbsSurface::try_new(
            3,
            1,
            7,
            2,
            controls,
            vec![0.0, 0.0, 1.0, 3.0, 6.0, 10.0, 11.0, 13.0, 16.0, 20.0, 20.0],
            vec![0.0, 0.0, 5.0, 5.0],
        )
        .unwrap();
        assert!(surface.is_periodic_u());

        let uniform = surface.try_make_uniform(SurfaceKnotDirection::U).unwrap();

        assert_eq!(
            uniform.knots_u(),
            &[0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0]
        );
        assert_eq!(uniform.knots_v(), &[0.0, 0.0, 5.0, 5.0]);
        assert_eq!(uniform.domain_u(), 2.0..=6.0);
        assert!(uniform.is_periodic_u());
        assert!(!uniform.is_periodic_v());
        assert_eq!(
            uniform
                .try_make_non_periodic(SurfaceKnotDirection::V)
                .unwrap(),
            uniform
        );
        assert!(matches!(
            uniform.try_insert_knot_u(4.5, 0),
            Err(GeometryError::InvalidKnotMultiplicity {
                actual: 0,
                maximum: 3
            })
        ));
        assert!(matches!(
            uniform.try_insert_knot_u(4.5, 4),
            Err(GeometryError::InvalidKnotMultiplicity {
                actual: 4,
                maximum: 3
            })
        ));
        assert!(matches!(
            uniform.try_insert_knot_u(2.0, 2),
            Err(GeometryError::InvalidEndpointKnotMultiplicity {
                actual: 2,
                degree: 3
            })
        ));

        let clamped = uniform
            .try_make_non_periodic(SurfaceKnotDirection::U)
            .unwrap();
        assert!(!clamped.is_periodic_u());
        assert!(!clamped.is_periodic_v());
        assert_eq!(clamped.domain_u(), uniform.domain_u());
        assert_eq!(clamped.domain_v(), uniform.domain_v());
        assert!(
            clamped.knots_u()[..=clamped.degree_u()]
                .iter()
                .all(|knot| *knot == *uniform.domain_u().start())
        );
        assert!(
            clamped.knots_u()[clamped.knots_u().len() - clamped.degree_u() - 1..]
                .iter()
                .all(|knot| *knot == *uniform.domain_u().end())
        );
        for u_index in 0..=16 {
            let u = 2.0 + u_index as Real / 4.0;
            for v in [0.0, 1.25, 5.0] {
                assert_point_near(
                    clamped.evaluate(u, v).unwrap(),
                    uniform.evaluate(u, v).unwrap(),
                );
            }
        }

        let refined = uniform.try_insert_knot_u(4.5, 1).unwrap();
        assert_eq!(refined.control_point_count_u(), 8);
        assert_eq!(
            refined.knots_u(),
            &[0.5, 0.5, 1.0, 2.0, 3.0, 4.0, 4.5, 5.0, 6.0, 7.0, 8.0, 8.0]
        );
        assert!(refined.is_periodic_u());
        for u_index in 0..=16 {
            let u = 2.0 + u_index as Real / 4.0;
            for v in [0.0, 1.25, 5.0] {
                assert_point_near(
                    refined.evaluate(u, v).unwrap(),
                    uniform.evaluate(u, v).unwrap(),
                );
            }
        }
    }

    #[test]
    fn surface_insertion_uses_the_full_homogeneous_net_for_periodicity() {
        let row = [
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
        ];
        let points = row
            .into_iter()
            .chain(row.into_iter().map(|point| {
                point
                    .translated(Vector3::try_new(0.0, 0.0, 3.0).unwrap())
                    .unwrap()
            }))
            .collect::<Vec<_>>();
        let weights = [
            1.0, 1.2, 0.8, 1.5, 2.0, 0.7, 1.8, 1.1, 0.9, 1.3, 0.6, 1.7, 1.4, 0.75,
        ];
        let controls = points
            .into_iter()
            .zip(weights)
            .map(|(point, weight)| WeightedPoint3::try_new(point, weight).unwrap())
            .collect();
        let surface = NurbsSurface::try_new_rational(
            3,
            1,
            7,
            2,
            controls,
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
            vec![0.0, 0.0, 5.0, 5.0],
        )
        .unwrap();
        assert!(surface.is_periodic_u());
        assert!(!surface.insertion_curve_is_periodic_u());

        let refined = surface.try_insert_knot_u(4.5, 1).unwrap();
        assert_eq!(
            refined.knots_u(),
            &[0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.5, 5.0, 6.0, 7.0, 8.0, 8.0]
        );
        assert!(!refined.is_periodic_u());
        for u_index in 0..=16 {
            let u = 2.0 + u_index as Real / 4.0;
            for v in [0.0, 1.25, 5.0] {
                assert_point_near(
                    refined.evaluate(u, v).unwrap(),
                    surface.evaluate(u, v).unwrap(),
                );
            }
        }
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
    fn control_polygon_mesh_matches_rhino_quads_periodic_seams_and_poles() {
        let bilinear = NurbsSurface::try_bilinear([
            point(0.0, 10.0, 0.0),
            point(3.0, 10.0, 0.0),
            point(3.0, 12.0, 1.0),
            point(0.0, 12.0, 0.0),
        ])
        .unwrap()
        .control_polygon_mesh(Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
        assert_eq!(
            bilinear.vertices(),
            &[
                point(0.0, 10.0, 0.0),
                point(3.0, 10.0, 0.0),
                point(0.0, 12.0, 0.0),
                point(3.0, 12.0, 1.0),
            ]
        );
        assert_eq!(bilinear.faces(), &[MeshFace::Quad([0, 1, 3, 2])]);
        assert_eq!(bilinear.topology().edge_count(), 4);

        let periodic = NurbsSurface::try_new(
            2,
            1,
            5,
            2,
            vec![
                point(20.0, 10.0, 0.0),
                point(22.0, 10.0, 0.0),
                point(21.0, 12.0, 0.0),
                point(20.0, 10.0, 0.0),
                point(22.0, 10.0, 0.0),
                point(20.0, 10.0, 3.0),
                point(22.0, 10.0, 3.0),
                point(21.0, 12.0, 3.0),
                point(20.0, 10.0, 3.0),
                point(22.0, 10.0, 3.0),
            ],
            vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap()
        .control_polygon_mesh(Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
        assert_eq!(
            periodic.vertices(),
            &[
                point(22.0, 10.0, 0.0),
                point(21.0, 12.0, 0.0),
                point(20.0, 10.0, 0.0),
                point(22.0, 10.0, 0.0),
                point(22.0, 10.0, 3.0),
                point(21.0, 12.0, 3.0),
                point(20.0, 10.0, 3.0),
                point(22.0, 10.0, 3.0),
            ]
        );
        assert_eq!(
            periodic.faces(),
            &[
                MeshFace::Quad([0, 1, 5, 4]),
                MeshFace::Quad([1, 2, 6, 5]),
                MeshFace::Quad([2, 3, 7, 6]),
            ]
        );
        assert_eq!(periodic.topology().edge_count(), 9);

        let frame = Frame3::try_from_normal(
            point(40.0, 10.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let sphere = NurbsSurface::try_sphere(frame, 2.0)
            .unwrap()
            .control_polygon_mesh(Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(sphere.vertices().len(), 45);
        assert_eq!(sphere.face_count(), 32);
        assert_eq!(
            sphere
                .faces()
                .iter()
                .filter(|face| face.is_triangle())
                .count(),
            16
        );
        assert_eq!(
            sphere.faces().iter().filter(|face| face.is_quad()).count(),
            16
        );
        assert_eq!(sphere.faces()[0], MeshFace::Triangle([0, 10, 9]));
        assert_eq!(sphere.faces()[24], MeshFace::Triangle([27, 28, 36]));
        assert_eq!(sphere.topology().edge_count(), 56);
    }

    #[test]
    fn control_polygon_mesh_only_snaps_coincident_clamped_sides() {
        let nearly_collapsed = NurbsSurface::try_new(
            1,
            2,
            2,
            4,
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0e-10, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(1.0, 2.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(1.0, 3.0, 0.0),
            ],
            vec![0.0, 0.0, 1.0, 1.0],
            vec![-1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        )
        .unwrap();
        let strict = Tolerance::try_new(1.0e-15, 1.0e-15, 1.0e-12).unwrap();
        let mesh = nearly_collapsed
            .control_polygon_mesh(strict)
            .unwrap()
            .unwrap();

        assert_eq!(mesh.face_count(), 3);
        assert!(mesh.faces().iter().all(|face| face.is_quad()));
        assert_eq!(mesh.vertices()[1], point(1.0e-10, 0.0, 0.0));
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
    fn polygon_meshing_preserves_quads_and_classifies_planarity() {
        let planar = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 6.0, 0.0),
            point(0.0, 6.0, 0.0),
        ])
        .unwrap();
        let plane = planar.plane(Tolerance::DEFAULT).unwrap().unwrap();
        assert_eq!(plane.origin().z(), 0.0);
        assert_eq!(plane.normal().z(), 1.0);
        let mesh = planar.polygon_mesh(0.5, false, Tolerance::DEFAULT).unwrap();
        assert_eq!(mesh.vertices().len(), 4);
        assert_eq!(mesh.faces(), &[MeshFace::Quad([0, 1, 3, 2])]);

        let twisted = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 6.0, 4.0),
            point(0.0, 6.0, 0.0),
        ])
        .unwrap();
        assert_eq!(twisted.plane(Tolerance::DEFAULT).unwrap(), None);
        let mesh = twisted.polygon_mesh(1.0, true, Tolerance::DEFAULT).unwrap();
        assert_eq!(mesh.face_count(), 1);
        assert!(mesh.faces()[0].is_quad());
    }

    #[test]
    fn curved_polygon_mesh_density_is_bounded_and_validated() {
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
        let coarse = surface
            .polygon_mesh(0.0, false, Tolerance::DEFAULT)
            .unwrap();
        let default = surface
            .polygon_mesh(0.5, false, Tolerance::DEFAULT)
            .unwrap();
        let dense = surface
            .polygon_mesh(1.0, false, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(coarse.face_count(), 16);
        assert_eq!(default.face_count(), 256);
        assert_eq!(dense.face_count(), 1024);
        assert!(dense.faces().iter().all(|face| face.is_quad()));
        for invalid in [-0.1, 1.1, Real::INFINITY] {
            assert_eq!(
                surface.polygon_mesh(invalid, false, Tolerance::DEFAULT),
                Err(GeometryError::InvalidMeshDensity(invalid))
            );
        }
        assert!(matches!(
            surface.polygon_mesh(Real::NAN, false, Tolerance::DEFAULT),
            Err(GeometryError::InvalidMeshDensity(value)) if value.is_nan()
        ));
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
        let polygon_mesh = singular_boundary
            .polygon_mesh(0.5, false, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(polygon_mesh.faces().len(), 1);
        assert!(polygon_mesh.faces()[0].is_triangle());
    }

    #[test]
    fn tensor_surface_splits_preserve_rational_nonclamped_evaluation() {
        let mut controls = Vec::new();
        for v in 0..4 {
            for u in 0..4 {
                controls.push(
                    WeightedPoint3::try_new(
                        point(
                            u as Real * 2.0 + v as Real * 0.25,
                            v as Real * 1.5 - u as Real * 0.4,
                            (u * v) as Real * 0.3,
                        ),
                        0.5 + (u + 2 * v) as Real * 0.2,
                    )
                    .unwrap(),
                );
            }
        }
        let surface = NurbsSurface::try_new_rational(
            2,
            2,
            4,
            4,
            controls,
            vec![-2.0, -1.0, 0.0, 0.7, 2.0, 3.0, 4.0],
            vec![8.0, 9.0, 10.0, 11.5, 14.0, 15.0, 16.0],
        )
        .unwrap();

        let (left, right) = surface.try_split_u(0.9).unwrap();
        assert_eq!(left.domain_u(), 0.0..=0.9);
        assert_eq!(right.domain_u(), 0.9..=2.0);
        assert_eq!(left.knots_v(), surface.knots_v());
        assert_eq!(right.knots_v(), surface.knots_v());
        assert!(
            left.knots_u()[left.knots_u().len() - 3..]
                .iter()
                .all(|knot| *knot == 0.9)
        );
        assert!(right.knots_u()[..3].iter().all(|knot| *knot == 0.9));
        for v in [10.0, 10.8, 12.6, 14.0] {
            for u in [0.0, 0.3, 0.9, 1.4, 2.0] {
                let piece = if u <= 0.9 { &left } else { &right };
                assert_point_near(
                    piece.evaluate(u, v).unwrap(),
                    surface.evaluate(u, v).unwrap(),
                );
                let (_, actual_u, actual_v) = piece.evaluate_with_derivatives(u, v).unwrap();
                let (_, expected_u, expected_v) = surface.evaluate_with_derivatives(u, v).unwrap();
                for (actual, expected) in [
                    (actual_u.x(), expected_u.x()),
                    (actual_u.y(), expected_u.y()),
                    (actual_u.z(), expected_u.z()),
                    (actual_v.x(), expected_v.x()),
                    (actual_v.y(), expected_v.y()),
                    (actual_v.z(), expected_v.z()),
                ] {
                    assert!(Tolerance::DEFAULT.approx_eq(actual, expected));
                }
            }
        }

        let (low, high) = surface.try_split_v(12.2).unwrap();
        assert_eq!(low.domain_v(), 10.0..=12.2);
        assert_eq!(high.domain_v(), 12.2..=14.0);
        assert_eq!(low.knots_u(), surface.knots_u());
        assert_eq!(high.knots_u(), surface.knots_u());
        for u in [0.0, 0.6, 1.3, 2.0] {
            for v in [10.0, 11.0, 12.2, 13.0, 14.0] {
                let piece = if v <= 12.2 { &low } else { &high };
                assert_point_near(
                    piece.evaluate(u, v).unwrap(),
                    surface.evaluate(u, v).unwrap(),
                );
            }
        }
    }

    #[test]
    fn rectangular_surface_trim_preserves_parameters_and_clamps_both_directions() {
        let surface = NurbsSurface::try_new(
            2,
            2,
            4,
            4,
            (0..4)
                .flat_map(|v| {
                    (0..4).map(move |u| {
                        point(
                            u as Real + v as Real * 0.2,
                            v as Real - u as Real * 0.1,
                            (u * v) as Real * 0.25,
                        )
                    })
                })
                .collect(),
            vec![-2.0, -1.0, 0.0, 0.8, 2.0, 3.0, 4.0],
            vec![8.0, 9.0, 10.0, 11.0, 14.0, 15.0, 16.0],
        )
        .unwrap();
        let trimmed = surface.try_trimmed(0.25..=1.6, 10.4..=13.2).unwrap();
        assert_eq!(trimmed.domain_u(), 0.25..=1.6);
        assert_eq!(trimmed.domain_v(), 10.4..=13.2);
        assert!(trimmed.knots_u()[..3].iter().all(|knot| *knot == 0.25));
        assert!(
            trimmed.knots_u()[trimmed.knots_u().len() - 3..]
                .iter()
                .all(|knot| *knot == 1.6)
        );
        assert!(trimmed.knots_v()[..3].iter().all(|knot| *knot == 10.4));
        assert!(
            trimmed.knots_v()[trimmed.knots_v().len() - 3..]
                .iter()
                .all(|knot| *knot == 13.2)
        );
        for u_sample in 0..=8 {
            let u_fraction = u_sample as Real / 8.0;
            let u = 0.25_f64.mul_add(1.0 - u_fraction, 1.6 * u_fraction);
            for v_sample in 0..=8 {
                let v_fraction = v_sample as Real / 8.0;
                let v = 10.4_f64.mul_add(1.0 - v_fraction, 13.2 * v_fraction);
                assert_point_near(
                    trimmed.evaluate(u, v).unwrap(),
                    surface.evaluate(u, v).unwrap(),
                );
            }
        }
        assert_eq!(
            surface
                .try_trimmed(surface.domain_u(), surface.domain_v())
                .unwrap(),
            surface
        );

        assert!(surface.try_split_u(0.0).is_err());
        assert!(surface.try_split_v(14.0).is_err());
        assert!(surface.try_trimmed_u(1.0..=0.5).is_err());
        assert!(surface.try_trimmed_v(9.0..=12.0).is_err());
    }

    #[test]
    fn splitting_periodic_surface_direction_produces_clamped_patches() {
        let profile = crate::NurbsCurve::try_control_point_curve_with_closure(
            3,
            vec![
                point(-3.0, 0.0, 0.0),
                point(-1.0, 3.0, 0.0),
                point(2.0, 4.0, 0.0),
                point(5.0, 1.0, 0.0),
                point(4.0, -3.0, 0.0),
                point(0.0, -4.0, 0.0),
            ],
            crate::ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        let surface = NurbsSurface::try_extruded_curve(
            &profile,
            Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 5.0).unwrap(),
        )
        .unwrap();
        assert!(surface.is_periodic_u());
        let split = surface.parameter_at_u(0.37).unwrap();
        let (left, right) = surface.try_split_u(split).unwrap();
        assert!(!left.is_periodic_u());
        assert!(!right.is_periodic_u());
        for u_sample in 0..=20 {
            let u = surface.parameter_at_u(u_sample as Real / 20.0).unwrap();
            for v in [0.0, 2.5, 5.0] {
                let piece = if u <= split { &left } else { &right };
                assert_point_near(
                    piece.evaluate(u, v).unwrap(),
                    surface.evaluate(u, v).unwrap(),
                );
            }
        }
        assert_eq!(surface.try_trimmed_u(surface.domain_u()).unwrap(), surface);
    }
}
