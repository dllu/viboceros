use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2};

mod closest;

use crate::{
    AffineTransform3, BoundingBox3, GeometryError, NurbsCurve, Point3, Real, Tolerance,
    UnitVector3, Vector3, WeightedPoint3, integration::integrate_adaptive, require_finite,
    vector::product_three,
};

/// A finite planar ellipse with two orthonormal principal-axis directions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipse3 {
    center: Point3,
    radius_x: Real,
    radius_y: Real,
    x_axis: UnitVector3,
    y_axis: UnitVector3,
    domain: [Real; 2],
}

impl Ellipse3 {
    /// Constructs an ellipse from an orthogonal frame and its semi-axis radii.
    /// Axes within the angular tolerance are orthogonalized before storage.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        center: Point3,
        radius_x: Real,
        radius_y: Real,
        x_axis: UnitVector3,
        y_axis: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite([radius_x, radius_y], "ellipse radii")?;
        if radius_x <= tolerance.absolute() || radius_y <= tolerance.absolute() {
            return Err(GeometryError::Degenerate { context: "ellipse" });
        }
        let dot = x_axis.as_vector().dot(y_axis.as_vector())?;
        if dot.abs() > tolerance.angular() {
            return Err(GeometryError::Degenerate {
                context: "ellipse axes",
            });
        }
        let y_axis = perpendicular_component(y_axis.as_vector(), x_axis)?.normalized(tolerance)?;
        let ellipse = Self {
            center,
            radius_x,
            radius_y,
            x_axis,
            y_axis,
            domain: [0.0, std::f64::consts::TAU],
        };
        ellipse.checked_bounds()?;
        Ok(ellipse)
    }

    /// Constructs RhinoCommon's three-point ellipse. The interactive Ellipse
    /// command instead measures the perpendicular second-axis component.
    /// `first_axis_point` fixes the X semi-axis. The component of
    /// `second_axis_point - center` perpendicular to it fixes the Y direction,
    /// while the full vector length fixes the Y radius. Consequently the
    /// third point is on the ellipse only when the two picked vectors are
    /// perpendicular, matching Rhino's three-point ellipse convention.
    pub fn try_from_three_points(
        center: Point3,
        first_axis_point: Point3,
        second_axis_point: Point3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let first = center.vector_to(first_axis_point)?;
        let radius_x = first.length()?;
        let x_axis = first.normalized(tolerance)?;
        let second = center.vector_to(second_axis_point)?;
        let radius_y = second.length()?;
        let perpendicular = perpendicular_component(second, x_axis)?;
        let y_axis = perpendicular.normalized(tolerance)?;
        Self::try_new(center, radius_x, radius_y, x_axis, y_axis, tolerance)
    }

    /// Ellipse through a point with two distinct foci. The focal line fixes X;
    /// the point chooses the plane and positive Y side.
    pub fn try_from_foci(
        first: Point3,
        second: Point3,
        point: Point3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::from_foci(first, second, point, None, tolerance)
    }

    /// Uses the construction plane to resolve the plane of an otherwise
    /// ambiguous collinear pick beyond the foci. Noncollinear picks still fix
    /// their own plane. Points between the foci cannot define a finite ellipse.
    pub fn try_from_foci_with_plane(
        first: Point3,
        second: Point3,
        point: Point3,
        plane: crate::Frame3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::from_foci(first, second, point, Some(plane), tolerance)
    }
    fn from_foci(
        first: Point3,
        second: Point3,
        point: Point3,
        fallback: Option<crate::Frame3>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let center = first.midpoint(second)?;
        let axis = first.vector_to(second)?;
        let frame =
            crate::Frame3::try_from_directions(center, axis, center.vector_to(point)?, tolerance)
                .or_else(|error| {
                let Some(plane) = fallback else {
                    return Err(error);
                };
                if perpendicular_component(center.vector_to(point)?, axis.normalized(tolerance)?)?
                    .length()?
                    > tolerance.absolute()
                {
                    return Err(error);
                }
                let y = plane.z_axis().as_vector().cross(axis)?;
                crate::Frame3::try_from_directions(
                    center,
                    axis,
                    if y.length()? > tolerance.absolute() {
                        y
                    } else {
                        plane.x_axis().as_vector().cross(axis)?
                    },
                    tolerance,
                )
            })?;
        let c = 0.5 * first.distance_to(second)?;
        let [x, y, _] = frame.coordinates_of(point)?;
        let left = x + c;
        let right = x - c;
        let r1 = left.hypot(y);
        let r2 = right.hypot(y);
        let a = 0.5 * r1 + 0.5 * r2;
        // Evaluate a-c without subtracting nearly equal focal distances. For
        // a thin ellipse this preserves the transverse radius even when a==c
        // after rounding. hypot(t,y)-abs(t) = y*(y/hypot)/(1+abs(t)/hypot).
        let excess = |t: Real, r: Real| {
            if r == 0. {
                0.
            } else {
                y.abs() * (y.abs() / r) / (1. + t.abs() / r)
            }
        };
        let outside = (x.abs() - c).max(0.);
        let delta = 0.5 * excess(left, r1) + 0.5 * excess(right, r2) + outside;
        let b = delta.sqrt() * a.sqrt() * (1. + c / a).sqrt();
        Self::try_new(center, a, b, frame.x_axis(), frame.y_axis(), tolerance)
    }

    /// Focus locations, ordered from negative to positive along the longer axis.
    pub fn foci(self) -> Result<[Point3; 2], GeometryError> {
        let (major, minor, axis) = if self.radius_x >= self.radius_y {
            (self.radius_x, self.radius_y, self.x_axis)
        } else {
            (self.radius_y, self.radius_x, self.y_axis)
        };
        let distance = (major - minor).sqrt() * major.sqrt() * (1. + minor / major).sqrt();
        let offset = axis.as_vector().scaled(distance)?;
        Ok([
            self.center.translated(offset.scaled(-1.)?)?,
            self.center.translated(offset)?,
        ])
    }

    #[inline]
    pub const fn center(self) -> Point3 {
        self.center
    }

    #[inline]
    pub const fn radius_x(self) -> Real {
        self.radius_x
    }

    #[inline]
    pub const fn radius_y(self) -> Real {
        self.radius_y
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
        require_finite([angle_radians], "ellipse angle")?;
        let (sine, cosine) = angle_radians.sin_cos();
        self.frame_point(cosine, sine, 1.0)
    }

    pub fn domain(self) -> std::ops::RangeInclusive<Real> {
        self.domain[0]..=self.domain[1]
    }

    pub fn try_reparameterized(
        self,
        domain: std::ops::RangeInclusive<Real>,
    ) -> Result<Self, GeometryError> {
        crate::parameter::check_interval(&domain)?;
        Ok(Self {
            domain: [*domain.start(), *domain.end()],
            ..self
        })
    }

    /// Native rational-quadratic parameterization, not the drafting angle.
    pub fn evaluate(self, parameter: Real) -> Result<Point3, GeometryError> {
        let jet = crate::curve_evaluate::ellipse_unit_jet(parameter, self.domain())?;
        self.frame_point(jet[0][0], jet[0][1], 1.0)
    }

    pub fn quadrants(self) -> Result<[Point3; 4], GeometryError> {
        Ok([
            self.frame_point(1.0, 0.0, 1.0)?,
            self.frame_point(0.0, 1.0, 1.0)?,
            self.frame_point(-1.0, 0.0, 1.0)?,
            self.frame_point(0.0, -1.0, 1.0)?,
        ])
    }

    pub fn length(self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        if self.radius_x == self.radius_y {
            let length = std::f64::consts::TAU * self.radius_x;
            require_finite([length], "ellipse length")?;
            return Ok(length);
        }
        let quadrant = integrate_adaptive(
            0.0,
            FRAC_PI_2,
            tolerance.absolute() * 0.25,
            tolerance.relative(),
            |angle| {
                let (sine, cosine) = angle.sin_cos();
                let speed = (self.radius_x * sine).hypot(self.radius_y * cosine);
                require_finite([speed], "ellipse speed")?;
                Ok(speed)
            },
        )?;
        let length = quadrant * 4.0;
        require_finite([length], "ellipse length")?;
        Ok(length)
    }

    pub fn area(self) -> Result<Real, GeometryError> {
        let area = product_three(
            std::f64::consts::PI,
            self.radius_x,
            self.radius_y,
            "ellipse area",
        )?;
        require_finite([area], "ellipse area")?;
        Ok(area)
    }

    /// Reverses the parameter direction while retaining the seam point.
    pub fn reversed(self) -> Self {
        Self {
            y_axis: self.y_axis.opposite(),
            domain: [-self.domain[1], -self.domain[0]],
            ..self
        }
    }

    pub fn bounds(self) -> BoundingBox3 {
        self.checked_bounds()
            .expect("a validated ellipse has finite bounds")
    }

    fn checked_bounds(self) -> Result<BoundingBox3, GeometryError> {
        let x = self.x_axis.as_vector().to_array();
        let y = self.y_axis.as_vector().to_array();
        let center = self.center.to_array();
        let mut min = [0.0; 3];
        let mut max = [0.0; 3];
        for coordinate in 0..3 {
            let extent = (self.radius_x * x[coordinate]).hypot(self.radius_y * y[coordinate]);
            min[coordinate] = center[coordinate] - extent;
            max[coordinate] = center[coordinate] + extent;
        }
        require_finite(min.into_iter().chain(max), "ellipse bounds")?;
        BoundingBox3::from_points([Point3::try_from(min)?, Point3::try_from(max)?])
    }

    /// Returns the exact four-span rational quadratic representation.
    pub fn to_nurbs(self) -> Result<NurbsCurve, GeometryError> {
        let controls = [
            (1.0, 0.0),
            (1.0, 1.0),
            (0.0, 1.0),
            (-1.0, 1.0),
            (-1.0, 0.0),
            (-1.0, -1.0),
            (0.0, -1.0),
            (1.0, -1.0),
            (1.0, 0.0),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (x, y))| {
            WeightedPoint3::try_new(
                self.frame_point(x, y, 1.0)?,
                if i % 2 == 0 { 1.0 } else { FRAC_1_SQRT_2 },
            )
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
        NurbsCurve::try_new_rational(
            2,
            controls,
            vec![
                0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
            ],
        )?
        .try_reparameterized(self.domain())
    }

    /// Preserves the analytic representation if transformed axes remain
    /// orthogonal. Callers can promote `None` to the exact NURBS image.
    pub fn transformed_orthogonal(
        self,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let transformed_x =
            transform.transform_vector(self.x_axis.as_vector().scaled(self.radius_x)?)?;
        let transformed_y =
            transform.transform_vector(self.y_axis.as_vector().scaled(self.radius_y)?)?;
        let radius_x = transformed_x.length()?;
        let radius_y = transformed_y.length()?;
        let x_axis = transformed_x.normalized(tolerance)?;
        let y_axis = transformed_y.normalized(tolerance)?;
        if x_axis.as_vector().dot(y_axis.as_vector())?.abs() > tolerance.angular() {
            return Ok(None);
        }
        Self::try_new(
            transform.transform_point(self.center)?,
            radius_x,
            radius_y,
            x_axis,
            y_axis,
            tolerance,
        )
        .and_then(|ellipse| ellipse.try_reparameterized(self.domain()))
        .map(Some)
    }

    fn frame_point(
        self,
        cosine: Real,
        sine: Real,
        middle_weight: Real,
    ) -> Result<Point3, GeometryError> {
        require_finite([cosine, sine, middle_weight], "ellipse frame coordinates")?;
        let inverse_weight = 1.0 / middle_weight;
        let center = self.center.to_array();
        let x_axis = self.x_axis.as_vector().to_array();
        let y_axis = self.y_axis.as_vector().to_array();
        Point3::try_from(std::array::from_fn(|coordinate| {
            (self.radius_x * x_axis[coordinate]).mul_add(
                cosine * inverse_weight,
                (self.radius_y * y_axis[coordinate])
                    .mul_add(sine * inverse_weight, center[coordinate]),
            )
        }))
    }
}

fn perpendicular_component(vector: Vector3, axis: UnitVector3) -> Result<Vector3, GeometryError> {
    let axis = axis.as_vector();
    let component = vector.dot(axis)?;
    let parallel = axis.scaled(component)?;
    Vector3::try_new(
        vector.x() - parallel.x(),
        vector.y() - parallel.y(),
        vector.z() - parallel.z(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn axis(x: Real, y: Real, z: Real) -> UnitVector3 {
        UnitVector3::try_new(x, y, z, Tolerance::DEFAULT).unwrap()
    }

    #[test]
    fn focal_construction_uses_the_picked_plane_and_reproduces_distance_sum() {
        let first = point(1., 2., 3.);
        let second = point(7., 10., 3.);
        let through = point(4., 6., 7.);
        let ellipse = Ellipse3::try_from_foci(first, second, through, Tolerance::DEFAULT).unwrap();
        assert_eq!(ellipse.center(), point(4., 6., 3.));
        assert!((ellipse.radius_x() - 41_f64.sqrt()).abs() < 1e-14);
        assert!((ellipse.radius_y() - 4.).abs() < 1e-14);
        assert!(
            ellipse.quadrants().unwrap()[1]
                .distance_to(through)
                .unwrap()
                < 1e-14
        );
        let foci = ellipse.foci().unwrap();
        assert!(foci[0].distance_to(first).unwrap() < 1e-13);
        assert!(foci[1].distance_to(second).unwrap() < 1e-13);
        for i in 0..32 {
            let p = ellipse
                .point_at_angle(std::f64::consts::TAU * i as f64 / 32.)
                .unwrap();
            let sum = p.distance_to(first).unwrap() + p.distance_to(second).unwrap();
            assert!((sum - 2. * ellipse.radius_x()).abs() < 1e-12);
        }
    }
    #[test]
    fn thin_focal_ellipses_do_not_lose_the_minor_radius_to_subtraction() {
        let ellipse = Ellipse3::try_from_foci(
            point(-1e8, 0., 0.),
            point(1e8, 0., 0.),
            point(0., 1e-3, 0.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(ellipse.radius_x(), 1e8);
        assert!((ellipse.radius_y() - 1e-3).abs() < 1e-18);
        assert!(
            ellipse
                .to_nurbs()
                .unwrap()
                .evaluate(std::f64::consts::FRAC_PI_2)
                .unwrap()
                .distance_to(point(0., 1e-3, 0.))
                .unwrap()
                < 1e-12
        );
        for focus_pair in [
            (point(0., 0., 0.), point(0., 0., 0.)),
            (point(-1., 0., 0.), point(1., 0., 0.)),
        ] {
            assert!(
                Ellipse3::try_from_foci(
                    focus_pair.0,
                    focus_pair.1,
                    point(2., 0., 0.),
                    Tolerance::DEFAULT
                )
                .is_err()
            );
        }
    }

    #[test]
    fn three_points_project_the_second_direction_and_retain_radius() {
        let ellipse = Ellipse3::try_from_three_points(
            point(1.0, 2.0, 3.0),
            point(5.0, 2.0, 3.0),
            point(3.0, -4.0, 3.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(ellipse.radius_x(), 4.0);
        let radius_y = 40.0_f64.sqrt();
        assert_eq!(ellipse.radius_y(), radius_y);
        assert_eq!(ellipse.quadrants().unwrap()[0], point(5.0, 2.0, 3.0));
        assert_eq!(
            ellipse.quadrants().unwrap()[1],
            point(1.0, 2.0 - radius_y, 3.0)
        );
        assert_eq!(ellipse.normal().unwrap(), axis(0.0, 0.0, -1.0));
    }

    #[test]
    fn bounds_are_exact_for_a_rotated_frame() {
        let inverse_sqrt_two = FRAC_1_SQRT_2;
        let ellipse = Ellipse3::try_new(
            point(2.0, -3.0, 5.0),
            4.0,
            2.0,
            axis(inverse_sqrt_two, inverse_sqrt_two, 0.0),
            axis(-inverse_sqrt_two, inverse_sqrt_two, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let extent = 10.0_f64.sqrt();
        assert!(Tolerance::DEFAULT.approx_eq(ellipse.bounds().min().x(), 2.0 - extent));
        assert!(Tolerance::DEFAULT.approx_eq(ellipse.bounds().max().y(), -3.0 + extent));
        assert_eq!(ellipse.bounds().min().z(), 5.0);
        for sample in 0..360 {
            let point = ellipse
                .point_at_angle(TAU * sample as Real / 360.0)
                .unwrap();
            assert!(point.x() >= ellipse.bounds().min().x());
            assert!(point.x() <= ellipse.bounds().max().x());
            assert!(point.y() >= ellipse.bounds().min().y());
            assert!(point.y() <= ellipse.bounds().max().y());
        }
    }

    #[test]
    fn rational_quadratic_is_an_exact_ellipse() {
        let ellipse = Ellipse3::try_new(
            point(1.0, -2.0, 3.0),
            5.0,
            2.0,
            axis(1.0, 0.0, 0.0),
            axis(0.0, 1.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let curve = ellipse.to_nurbs().unwrap();
        assert_eq!(curve.degree(), 2);
        assert_eq!(curve.control_points().len(), 9);
        for sample in 0..=256 {
            let point = curve
                .evaluate(curve.parameter_at(sample as Real / 256.0).unwrap())
                .unwrap();
            let x = (point.x() - 1.0) / 5.0;
            let y = (point.y() + 2.0) / 2.0;
            assert!(Tolerance::DEFAULT.approx_eq(x.mul_add(x, y * y), 1.0));
            assert!(Tolerance::DEFAULT.approx_eq(point.z(), 3.0));
        }
    }

    #[test]
    fn computes_perimeter_and_area_to_requested_accuracy() {
        let ellipse = Ellipse3::try_new(
            point(1.0, -2.0, 3.0),
            5.0,
            2.0,
            axis(1.0, 0.0, 0.0),
            axis(0.0, 1.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(
            Tolerance::try_new(1.0e-11, 1.0e-12, 1.0e-12)
                .unwrap()
                .approx_eq(
                    ellipse.length(Tolerance::DEFAULT).unwrap(),
                    23.013_112_595_664_843
                )
        );
        assert!(Tolerance::DEFAULT.approx_eq(ellipse.area().unwrap(), 10.0 * std::f64::consts::PI));
        let reversed = ellipse.reversed();
        assert_eq!(
            reversed.point_at_angle(0.0).unwrap(),
            ellipse.point_at_angle(0.0).unwrap()
        );
        for sample in 0..=16 {
            let angle = std::f64::consts::TAU * sample as Real / 16.0;
            assert!(
                reversed
                    .point_at_angle(angle)
                    .unwrap()
                    .is_near(ellipse.point_at_angle(-angle).unwrap(), Tolerance::DEFAULT)
            );
        }

        let circle = Ellipse3::try_new(
            point(0.0, 0.0, 0.0),
            3.0,
            3.0,
            axis(1.0, 0.0, 0.0),
            axis(0.0, 1.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            circle.length(Tolerance::DEFAULT).unwrap(),
            std::f64::consts::TAU * 3.0
        );

        let slender = Ellipse3::try_new(
            point(0.0, 0.0, 0.0),
            1.0e307,
            2.0e-9,
            axis(1.0, 0.0, 0.0),
            axis(0.0, 1.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(slender.area().unwrap().is_finite());
        assert!(Tolerance::DEFAULT.approx_eq(
            slender.area().unwrap() / 1.0e298,
            2.0 * std::f64::consts::PI
        ));
    }

    #[test]
    fn orthogonal_transforms_preserve_analytics_and_shear_promotes() {
        let ellipse = Ellipse3::try_new(
            point(0.0, 0.0, 0.0),
            4.0,
            2.0,
            axis(1.0, 0.0, 0.0),
            axis(0.0, 1.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let scale = AffineTransform3::try_new(
            [[3.0, 0.0, 0.0], [0.0, 5.0, 0.0], [0.0, 0.0, 1.0]],
            Vector3::try_new(7.0, 11.0, 13.0).unwrap(),
        )
        .unwrap();
        let scaled = ellipse
            .transformed_orthogonal(scale, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(scaled.center(), point(7.0, 11.0, 13.0));
        assert_eq!(scaled.radius_x(), 12.0);
        assert_eq!(scaled.radius_y(), 10.0);

        let shear = AffineTransform3::try_new(
            [[1.0, 0.5, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        )
        .unwrap();
        assert!(
            ellipse
                .transformed_orthogonal(shear, Tolerance::DEFAULT)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn rejects_degenerate_nonorthogonal_and_overflowing_inputs() {
        assert!(
            Ellipse3::try_new(
                point(0.0, 0.0, 0.0),
                0.0,
                2.0,
                axis(1.0, 0.0, 0.0),
                axis(0.0, 1.0, 0.0),
                Tolerance::DEFAULT,
            )
            .is_err()
        );
        assert!(
            Ellipse3::try_new(
                point(0.0, 0.0, 0.0),
                2.0,
                1.0,
                axis(1.0, 0.0, 0.0),
                axis(1.0, 1.0, 0.0),
                Tolerance::DEFAULT,
            )
            .is_err()
        );
        assert!(
            Ellipse3::try_new(
                point(Real::MAX, 0.0, 0.0),
                Real::MAX,
                1.0,
                axis(1.0, 0.0, 0.0),
                axis(0.0, 1.0, 0.0),
                Tolerance::DEFAULT,
            )
            .is_err()
        );
    }
}
