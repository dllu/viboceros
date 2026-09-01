use std::f64::consts::{FRAC_PI_2, PI, TAU};

use crate::{
    AffineTransform3, BoundingBox3, GeometryError, NurbsCurve, Point3, Real, Tolerance,
    UnitVector3, Vector3, WeightedPoint3, require_finite,
};

/// A finite, non-degenerate circle in three-dimensional model space.
///
/// The orthonormal in-plane axes retain a stable parameterization, which is
/// useful for quadrant snaps and exact rational NURBS conversion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle3 {
    center: Point3,
    radius: Real,
    x_axis: UnitVector3,
    y_axis: UnitVector3,
}

impl Circle3 {
    /// Constructs a circle with a deterministic in-plane frame.
    pub fn try_new(
        center: Point3,
        radius: Real,
        normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite([radius], "circle radius")?;
        if radius <= tolerance.absolute() {
            return Err(GeometryError::Degenerate { context: "circle" });
        }

        let [nx, ny, nz] = normal.as_vector().to_array().map(Real::abs);
        let reference = if nx <= ny && nx <= nz {
            Vector3::try_new(1.0, 0.0, 0.0)?
        } else if ny <= nz {
            Vector3::try_new(0.0, 1.0, 0.0)?
        } else {
            Vector3::try_new(0.0, 0.0, 1.0)?
        };
        let x_axis = projected_unit(reference, normal, tolerance)?;
        Self::try_from_frame(center, radius, x_axis, normal, tolerance)
    }

    /// Constructs a circle whose zero-angle direction points from `center`
    /// toward the in-plane projection of `point_on_circle`.
    pub fn try_from_center_point(
        center: Point3,
        point_on_circle: Point3,
        normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let radial = center.vector_to(point_on_circle)?;
        let projected = project_to_plane(radial, normal)?;
        let radius = projected.length()?;
        let x_axis = projected.normalized(tolerance)?;
        Self::try_from_frame(center, radius, x_axis, normal, tolerance)
    }

    fn try_from_frame(
        center: Point3,
        radius: Real,
        x_axis: UnitVector3,
        normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite([radius], "circle radius")?;
        if radius <= tolerance.absolute() {
            return Err(GeometryError::Degenerate { context: "circle" });
        }
        let x_axis = projected_unit(x_axis.as_vector(), normal, tolerance)?;
        let y_axis = normal
            .as_vector()
            .cross(x_axis.as_vector())?
            .normalized_nonzero()?;
        let circle = Self {
            center,
            radius,
            x_axis,
            y_axis,
        };
        // Validate the exact coordinate extrema once so all later evaluations
        // are guaranteed to remain finite.
        circle.checked_bounds()?;
        Ok(circle)
    }

    #[inline]
    pub const fn center(self) -> Point3 {
        self.center
    }

    #[inline]
    pub const fn radius(self) -> Real {
        self.radius
    }

    #[inline]
    pub const fn x_axis(self) -> UnitVector3 {
        self.x_axis
    }

    #[inline]
    pub const fn y_axis(self) -> UnitVector3 {
        self.y_axis
    }

    pub fn normal(self) -> Result<UnitVector3, GeometryError> {
        self.x_axis
            .as_vector()
            .cross(self.y_axis.as_vector())?
            .normalized_nonzero()
    }

    pub fn point_at_angle(self, angle_radians: Real) -> Result<Point3, GeometryError> {
        require_finite([angle_radians], "circle angle")?;
        let (sine, cosine) = angle_radians.sin_cos();
        frame_point(
            self.center,
            self.x_axis,
            self.y_axis,
            self.radius,
            cosine,
            sine,
        )
    }

    pub fn quadrants(self) -> Result<[Point3; 4], GeometryError> {
        Ok([
            frame_point(self.center, self.x_axis, self.y_axis, self.radius, 1.0, 0.0)?,
            frame_point(self.center, self.x_axis, self.y_axis, self.radius, 0.0, 1.0)?,
            frame_point(
                self.center,
                self.x_axis,
                self.y_axis,
                self.radius,
                -1.0,
                0.0,
            )?,
            frame_point(
                self.center,
                self.x_axis,
                self.y_axis,
                self.radius,
                0.0,
                -1.0,
            )?,
        ])
    }

    pub fn length(self) -> Result<Real, GeometryError> {
        let length = TAU * self.radius;
        require_finite([length], "circle length")?;
        Ok(length)
    }

    pub fn area(self) -> Result<Real, GeometryError> {
        let area = PI * self.radius * self.radius;
        require_finite([area], "circle area")?;
        Ok(area)
    }

    pub fn bounds(self) -> BoundingBox3 {
        self.checked_bounds()
            .expect("a validated circle has finite bounds")
    }

    fn checked_bounds(self) -> Result<BoundingBox3, GeometryError> {
        let x = self.x_axis.as_vector().to_array();
        let y = self.y_axis.as_vector().to_array();
        let center = self.center.to_array();
        let mut min = [0.0; 3];
        let mut max = [0.0; 3];
        for coordinate in 0..3 {
            let extent = self.radius * x[coordinate].hypot(y[coordinate]);
            min[coordinate] = center[coordinate] - extent;
            max[coordinate] = center[coordinate] + extent;
        }
        require_finite(min.into_iter().chain(max), "circle bounds")?;
        BoundingBox3::from_points([Point3::try_from(min)?, Point3::try_from(max)?])
    }

    /// Returns an exact four-span rational quadratic representation.
    pub fn to_nurbs(self) -> Result<NurbsCurve, GeometryError> {
        circular_nurbs(self, TAU)
    }

    /// Preserves the analytic representation under a similarity. A general
    /// affine map returns `None`, allowing callers to promote the exact result
    /// to a NURBS ellipse instead of mislabelling it as a circle.
    pub fn transformed_similarity(
        self,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let Some((scale, x_axis, y_axis)) =
            transformed_frame(self.x_axis, self.y_axis, transform, tolerance)?
        else {
            return Ok(None);
        };
        let center = transform.transform_point(self.center)?;
        let radius = self.radius * scale;
        let normal = x_axis
            .as_vector()
            .cross(y_axis.as_vector())?
            .normalized_nonzero()?;
        Self::try_from_frame(center, radius, x_axis, normal, tolerance).map(Some)
    }
}

/// A finite circular arc with a positive sweep smaller than one revolution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CircularArc3 {
    circle: Circle3,
    sweep_radians: Real,
}

impl CircularArc3 {
    /// Constructs the unique oriented arc from `start` through `through` to
    /// `end`. Collinear or tolerance-coincident inputs are rejected.
    pub fn try_from_three_points(
        start: Point3,
        through: Point3,
        end: Point3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let start_to_through = start.vector_to(through)?;
        let start_to_end = start.vector_to(end)?;
        let local_x = start_to_through.normalized(tolerance)?;
        let end_direction = start_to_end.normalized(tolerance)?;
        let normal_vector = local_x.as_vector().cross(end_direction.as_vector())?;
        if normal_vector.length()? <= tolerance.angular() {
            return Err(GeometryError::Degenerate {
                context: "circular arc",
            });
        }
        let normal = normal_vector.normalized_nonzero()?;
        let local_y = normal
            .as_vector()
            .cross(local_x.as_vector())?
            .normalized_nonzero()?;

        let chord = start_to_through.length()?;
        let end_x = start_to_end.dot(local_x.as_vector())?;
        let end_y = start_to_end.dot(local_y.as_vector())?;
        let scale = chord.abs().max(end_x.abs()).max(end_y.abs());
        if scale == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "circular arc",
            });
        }
        let chord = chord / scale;
        let end_x = end_x / scale;
        let end_y = end_y / scale;
        if end_y.abs() <= tolerance.angular() {
            return Err(GeometryError::Degenerate {
                context: "circular arc",
            });
        }
        let center_x = chord * 0.5;
        let center_y = end_x.mul_add(end_x - chord, end_y * end_y) / (2.0 * end_y);
        let local_center = frame_point(start, local_x, local_y, scale, center_x, center_y)?;
        let circle = Circle3::try_from_center_point(local_center, start, normal, tolerance)?;
        let through_angle = circle_angle(circle, through)?;
        let sweep_radians = circle_angle(circle, end)?;
        if !(through_angle > 0.0 && through_angle < sweep_radians && sweep_radians < TAU) {
            return Err(GeometryError::Degenerate {
                context: "circular arc",
            });
        }
        Ok(Self {
            circle,
            sweep_radians,
        })
    }

    #[inline]
    pub const fn center(self) -> Point3 {
        self.circle.center()
    }

    #[inline]
    pub const fn radius(self) -> Real {
        self.circle.radius()
    }

    #[inline]
    pub const fn sweep_radians(self) -> Real {
        self.sweep_radians
    }

    pub fn normal(self) -> Result<UnitVector3, GeometryError> {
        self.circle.normal()
    }

    pub fn start(self) -> Result<Point3, GeometryError> {
        self.circle.point_at_angle(0.0)
    }

    pub fn end(self) -> Result<Point3, GeometryError> {
        self.circle.point_at_angle(self.sweep_radians)
    }

    pub fn point_at(self, normalized: Real) -> Result<Point3, GeometryError> {
        require_finite([normalized], "arc parameter")?;
        if !(0.0..=1.0).contains(&normalized) {
            return Err(GeometryError::ParameterOutOfDomain {
                parameter: normalized,
                domain_start: 0.0,
                domain_end: 1.0,
            });
        }
        self.circle.point_at_angle(self.sweep_radians * normalized)
    }

    pub fn length(self) -> Result<Real, GeometryError> {
        let length = self.radius() * self.sweep_radians;
        require_finite([length], "arc length")?;
        Ok(length)
    }

    pub fn bounds(self) -> BoundingBox3 {
        let x = self.circle.x_axis.as_vector().to_array();
        let y = self.circle.y_axis.as_vector().to_array();
        let mut points = vec![
            self.start().expect("a validated arc has a start point"),
            self.end().expect("a validated arc has an end point"),
        ];
        for coordinate in 0..3 {
            let maximum_angle = positive_angle(y[coordinate].atan2(x[coordinate]));
            for angle in [maximum_angle, positive_angle(maximum_angle + PI)] {
                if angle > 0.0 && angle < self.sweep_radians {
                    points.push(
                        self.circle
                            .point_at_angle(angle)
                            .expect("a validated arc has finite extrema"),
                    );
                }
            }
        }
        BoundingBox3::from_points(points).expect("an arc has endpoints")
    }

    pub fn to_nurbs(self) -> Result<NurbsCurve, GeometryError> {
        circular_nurbs(self.circle, self.sweep_radians)
    }

    pub fn transformed_similarity(
        self,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        Ok(self
            .circle
            .transformed_similarity(transform, tolerance)?
            .map(|circle| Self {
                circle,
                sweep_radians: self.sweep_radians,
            }))
    }
}

fn circular_nurbs(circle: Circle3, sweep: Real) -> Result<NurbsCurve, GeometryError> {
    let span_count = (sweep / FRAC_PI_2).ceil() as usize;
    let span_angle = sweep / span_count as Real;
    let half_angle = span_angle * 0.5;
    let middle_weight = half_angle.cos();
    let mut controls = Vec::with_capacity(2 * span_count + 1);
    for span in 0..span_count {
        let start_angle = span as Real * span_angle;
        if span == 0 {
            controls.push(WeightedPoint3::try_new(
                circle.point_at_angle(start_angle)?,
                1.0,
            )?);
        }
        let middle_angle = start_angle + half_angle;
        let (middle_sine, middle_cosine) = middle_angle.sin_cos();
        controls.push(WeightedPoint3::try_new(
            frame_point(
                circle.center,
                circle.x_axis,
                circle.y_axis,
                circle.radius / middle_weight,
                middle_cosine,
                middle_sine,
            )?,
            middle_weight,
        )?);
        let end = if sweep == TAU && span + 1 == span_count {
            circle.point_at_angle(0.0)?
        } else {
            circle.point_at_angle(start_angle + span_angle)?
        };
        controls.push(WeightedPoint3::try_new(end, 1.0)?);
    }

    let mut knots = vec![0.0; 3];
    for span in 1..span_count {
        let knot = span as Real / span_count as Real;
        knots.extend([knot, knot]);
    }
    knots.extend([1.0; 3]);
    NurbsCurve::try_new_rational(2, controls, knots)
}

fn transformed_frame(
    x_axis: UnitVector3,
    y_axis: UnitVector3,
    transform: AffineTransform3,
    tolerance: Tolerance,
) -> Result<Option<(Real, UnitVector3, UnitVector3)>, GeometryError> {
    let x = transform.transform_vector(x_axis.as_vector())?;
    let y = transform.transform_vector(y_axis.as_vector())?;
    let x_length = x.length()?;
    let y_length = y.length()?;
    let largest_length = x_length.max(y_length);
    if x_length == 0.0
        || y_length == 0.0
        || (x_length - y_length).abs() > tolerance.relative() * largest_length
    {
        return Ok(None);
    }
    let x_axis = x.normalized_nonzero()?;
    let y_axis = y.normalized_nonzero()?;
    if x_axis.as_vector().dot(y_axis.as_vector())?.abs() > tolerance.angular() {
        return Ok(None);
    }
    Ok(Some((x_length * 0.5 + y_length * 0.5, x_axis, y_axis)))
}

fn circle_angle(circle: Circle3, point: Point3) -> Result<Real, GeometryError> {
    let radial = circle.center.vector_to(point)?;
    let x = radial.dot(circle.x_axis.as_vector())?;
    let y = radial.dot(circle.y_axis.as_vector())?;
    Ok(positive_angle(y.atan2(x)))
}

fn positive_angle(angle: Real) -> Real {
    let angle = angle.rem_euclid(TAU);
    if angle == TAU { 0.0 } else { angle }
}

fn projected_unit(
    vector: Vector3,
    normal: UnitVector3,
    tolerance: Tolerance,
) -> Result<UnitVector3, GeometryError> {
    project_to_plane(vector, normal)?.normalized(tolerance)
}

fn project_to_plane(vector: Vector3, normal: UnitVector3) -> Result<Vector3, GeometryError> {
    let normal_vector = normal.as_vector();
    let component = vector.dot(normal_vector)?;
    let normal_component = normal_vector.scaled(component)?;
    Vector3::try_new(
        vector.x() - normal_component.x(),
        vector.y() - normal_component.y(),
        vector.z() - normal_component.z(),
    )
}

fn frame_point(
    origin: Point3,
    x_axis: UnitVector3,
    y_axis: UnitVector3,
    scale: Real,
    x: Real,
    y: Real,
) -> Result<Point3, GeometryError> {
    require_finite([scale, x, y], "circular frame coordinates")?;
    let origin = origin.to_array();
    let x_axis = x_axis.as_vector().to_array();
    let y_axis = y_axis.as_vector().to_array();
    Point3::try_from(std::array::from_fn(|coordinate| {
        scale.mul_add(
            x_axis[coordinate].mul_add(x, y_axis[coordinate] * y),
            origin[coordinate],
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn z_axis() -> UnitVector3 {
        UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap()
    }

    #[test]
    fn circle_has_exact_frame_features_and_bounds() {
        let circle = Circle3::try_from_center_point(
            point(1.0, 2.0, 3.0),
            point(6.0, 2.0, 9.0),
            z_axis(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(circle.radius(), 5.0);
        assert_eq!(circle.point_at_angle(0.0).unwrap(), point(6.0, 2.0, 3.0));
        assert!(
            circle
                .point_at_angle(FRAC_PI_2)
                .unwrap()
                .is_near(point(1.0, 7.0, 3.0), Tolerance::DEFAULT)
        );
        assert_eq!(circle.bounds().min(), point(-4.0, -3.0, 3.0));
        assert_eq!(circle.bounds().max(), point(6.0, 7.0, 3.0));
        assert_eq!(circle.length().unwrap(), TAU * 5.0);
        assert_eq!(circle.area().unwrap(), PI * 25.0);
    }

    #[test]
    fn exact_circle_nurbs_stays_on_the_radius() {
        let circle =
            Circle3::try_new(point(2.0, -1.0, 4.0), 3.0, z_axis(), Tolerance::DEFAULT).unwrap();
        let curve = circle.to_nurbs().unwrap();
        assert_eq!(curve.degree(), 2);
        assert_eq!(curve.control_points().len(), 9);
        assert_eq!(curve.knots().len(), 12);
        for sample in 0..=64 {
            let point = curve.evaluate(sample as Real / 64.0).unwrap();
            assert!(
                Tolerance::DEFAULT
                    .approx_eq(point.distance_to(circle.center()).unwrap(), circle.radius())
            );
        }
    }

    #[test]
    fn three_points_choose_the_arc_that_contains_the_middle_point() {
        let arc = CircularArc3::try_from_three_points(
            point(1.0, 0.0, 0.0),
            point(0.0, -1.0, 0.0),
            point(-1.0, 0.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(arc.sweep_radians(), PI));
        assert!(
            arc.point_at(0.5)
                .unwrap()
                .is_near(point(0.0, -1.0, 0.0), Tolerance::DEFAULT)
        );
        assert!(
            arc.start()
                .unwrap()
                .is_near(point(1.0, 0.0, 0.0), Tolerance::DEFAULT)
        );
        assert!(
            arc.end()
                .unwrap()
                .is_near(point(-1.0, 0.0, 0.0), Tolerance::DEFAULT)
        );
    }

    #[test]
    fn three_points_retain_a_major_arc_instead_of_taking_the_short_path() {
        let arc = CircularArc3::try_from_three_points(
            point(1.0, 0.0, 0.0),
            point(0.0, -1.0, 0.0),
            point(0.0, 1.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(
            Tolerance::DEFAULT.approx_eq(arc.sweep_radians(), 3.0 * FRAC_PI_2),
            "three-point construction must pass through the supplied middle point"
        );
        let quarter_parameter = FRAC_PI_2 / arc.sweep_radians();
        assert!(
            arc.point_at(quarter_parameter)
                .unwrap()
                .is_near(point(0.0, -1.0, 0.0), Tolerance::DEFAULT)
        );
    }

    #[test]
    fn arc_nurbs_is_exact_and_bounds_include_interior_extrema() {
        let arc = CircularArc3::try_from_three_points(
            point(1.0, 0.0, 2.0),
            point(0.0, 1.0, 2.0),
            point(-1.0, 0.0, 2.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let bounds = arc.bounds();
        assert!(
            bounds
                .min()
                .is_near(point(-1.0, 0.0, 2.0), Tolerance::DEFAULT)
        );
        assert!(
            bounds
                .max()
                .is_near(point(1.0, 1.0, 2.0), Tolerance::DEFAULT)
        );
        let curve = arc.to_nurbs().unwrap();
        for sample in 0..=32 {
            let point = curve.evaluate(sample as Real / 32.0).unwrap();
            assert!(
                Tolerance::DEFAULT
                    .approx_eq(point.distance_to(arc.center()).unwrap(), arc.radius())
            );
        }
    }

    #[test]
    fn similarities_preserve_analytics_and_shear_is_detected() {
        let circle =
            Circle3::try_new(point(1.0, 2.0, 0.0), 3.0, z_axis(), Tolerance::DEFAULT).unwrap();
        let rotation =
            AffineTransform3::try_rotation(point(0.0, 0.0, 0.0), z_axis(), FRAC_PI_2).unwrap();
        let rotated = circle
            .transformed_similarity(rotation, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!(
            rotated
                .center()
                .is_near(point(-2.0, 1.0, 0.0), Tolerance::DEFAULT)
        );
        assert_eq!(rotated.radius(), 3.0);

        let shear = AffineTransform3::try_new(
            [[1.0, 0.5, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        )
        .unwrap();
        assert_eq!(
            circle
                .transformed_similarity(shear, Tolerance::DEFAULT)
                .unwrap(),
            None
        );

        let tiny_nonuniform = AffineTransform3::try_new(
            [[1.0e-12, 0.0, 0.0], [0.0, 2.0e-12, 0.0], [0.0, 0.0, 1.0]],
            Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        )
        .unwrap();
        assert_eq!(
            circle
                .transformed_similarity(tiny_nonuniform, Tolerance::DEFAULT)
                .unwrap(),
            None
        );
    }

    #[test]
    fn rejects_degenerate_or_non_finite_inputs() {
        assert!(Circle3::try_new(point(0.0, 0.0, 0.0), 0.0, z_axis(), Tolerance::DEFAULT).is_err());
        assert!(
            Circle3::try_new(
                point(0.0, 0.0, 0.0),
                Real::NAN,
                z_axis(),
                Tolerance::DEFAULT
            )
            .is_err()
        );
        assert!(
            CircularArc3::try_from_three_points(
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                Tolerance::DEFAULT,
            )
            .is_err()
        );
    }

    #[test]
    fn angular_degeneracy_is_independent_of_model_scale() {
        let radius = 1.0e-5;
        let arc = CircularArc3::try_from_three_points(
            point(radius, 0.0, 0.0),
            point(0.0, radius, 0.0),
            point(-radius, 0.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(arc.radius(), radius));
        assert!(Tolerance::DEFAULT.approx_eq(arc.sweep_radians(), PI));
    }
}
