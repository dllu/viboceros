//! A complete circular curve and its native parameter interval.

use crate::circular::CircleFrame3;
use crate::parameter::{check_interval, map_parameter};
use crate::{
    AffineTransform3, BoundingBox3, GeometryError, NurbsCurve, Point3, Real, Tolerance, UnitVector3,
};
use std::ops::RangeInclusive;

/// An analytic circle with an oriented frame and an independent native domain.
/// New curves use `[0,circumference]`; reversal and transforms retain the
/// corresponding parameterized curve, rather than reconstructing its domain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle3 {
    frame: CircleFrame3,
    domain: [Real; 2],
}

impl Circle3 {
    fn from_frame(frame: CircleFrame3) -> Result<Self, GeometryError> {
        let end = frame.length()?;
        check_interval(&(0.0..=end))?;
        Ok(Self {
            frame,
            domain: [0.0, end],
        })
    }
    pub fn try_new(
        center: Point3,
        radius: Real,
        normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::from_frame(CircleFrame3::try_new(center, radius, normal, tolerance)?)
    }
    pub fn try_from_center_point(
        center: Point3,
        point_on_circle: Point3,
        normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::from_frame(CircleFrame3::try_from_center_point(
            center,
            point_on_circle,
            normal,
            tolerance,
        )?)
    }
    pub fn try_from_frame(
        center: Point3,
        radius: Real,
        x_axis: UnitVector3,
        normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::from_frame(CircleFrame3::try_from_frame(
            center, radius, x_axis, normal, tolerance,
        )?)
    }
    pub(crate) const fn frame(self) -> CircleFrame3 {
        self.frame
    }
    pub const fn center(self) -> Point3 {
        self.frame.center()
    }
    pub const fn radius(self) -> Real {
        self.frame.radius()
    }
    pub const fn x_axis(self) -> UnitVector3 {
        self.frame.x_axis()
    }
    pub const fn y_axis(self) -> UnitVector3 {
        self.frame.y_axis()
    }
    pub fn normal(self) -> Result<UnitVector3, GeometryError> {
        self.frame.normal()
    }
    pub fn point_at_angle(self, angle: Real) -> Result<Point3, GeometryError> {
        self.frame.point_at_angle(angle)
    }
    pub fn quadrants(self) -> Result<[Point3; 4], GeometryError> {
        self.frame.quadrants()
    }
    pub fn bounds(self) -> BoundingBox3 {
        self.frame.bounds()
    }
    pub fn length(self) -> Result<Real, GeometryError> {
        self.frame.length()
    }
    pub fn area(self) -> Result<Real, GeometryError> {
        self.frame.area()
    }
    pub fn domain(self) -> RangeInclusive<Real> {
        self.domain[0]..=self.domain[1]
    }
    pub fn try_reparameterized(self, domain: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        check_interval(&domain)?;
        Ok(Self {
            domain: [*domain.start(), *domain.end()],
            ..self
        })
    }
    pub fn evaluate(self, parameter: Real) -> Result<Point3, GeometryError> {
        let angle = map_parameter(parameter, self.domain(), 0.0..=std::f64::consts::TAU)?;
        self.point_at_angle(if parameter == self.domain[1] {
            0.0
        } else {
            angle
        })
    }
    pub fn reversed(self) -> Self {
        Self {
            frame: self.frame.reversed(),
            domain: [-self.domain[1], -self.domain[0]],
        }
    }
    /// Exact rational locus in the native interval; angular speed is not
    /// retained by rational quadratic parameterization.
    pub fn to_nurbs(self) -> Result<NurbsCurve, GeometryError> {
        self.frame.to_nurbs()?.try_reparameterized(self.domain())
    }
    pub fn transformed_similarity(
        self,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        Ok(self
            .frame
            .transformed_similarity(transform, tolerance)?
            .map(|frame| Self {
                frame,
                domain: self.domain,
            }))
    }
}
