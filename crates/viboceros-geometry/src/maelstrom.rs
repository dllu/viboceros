//! Cylindrical spiral deformation inferred from public Maelstrom witnesses.
use crate::{
    AffineTransform3, Brep, Frame3, GeometryError, NurbsCurve, NurbsSurface, Point3, PointMorph,
    Real, Tolerance, Vector3, require_finite,
};

const SDK_ZERO: Real = 2.3283064365386963e-10;
const EQUAL_RADIUS: Real = 1.4901161193847656e-8;

#[derive(Clone, Copy, Debug)]
pub struct MaelstromPointMorph {
    frame: Frame3,
    radius0: Real,
    radius1: Real,
    angle: Real,
    near_equal: bool,
    cardinal_normal: Option<(usize, Real)>,
    preserve_structure: bool,
}
impl MaelstromPointMorph {
    /// Public SDK radius validity is independent of modeling tolerance.
    /// The frame normal defines the positive rotation direction.
    pub fn try_new(
        frame: Frame3,
        radius0: Real,
        radius1: Real,
        angle_radians: Real,
    ) -> Result<Self, GeometryError> {
        require_finite([radius0, radius1, angle_radians], "maelstrom definition")?;
        if radius0 <= SDK_ZERO || radius1 <= SDK_ZERO {
            return Err(GeometryError::Degenerate {
                context: "maelstrom radii",
            });
        }
        let normal = frame.z_axis().as_vector().to_array();
        let cardinal_normal = (0..3)
            .find(|i| normal[*i].abs() == 1. && (0..3).all(|j| j == *i || normal[j] == 0.))
            .map(|i| (i, normal[i]));
        Ok(Self {
            frame,
            radius0,
            radius1,
            angle: angle_radians,
            near_equal: (radius1 - radius0).abs() <= EQUAL_RADIUS * radius0,
            cardinal_normal,
            preserve_structure: false,
        })
    }
    pub fn with_preserve_structure(mut self, preserve: bool) -> Self {
        self.preserve_structure = preserve;
        self
    }
    pub fn is_identity(self) -> bool {
        self.angle == 0.
    }
    pub fn rigid_transform(self, center: Point3) -> Result<AffineTransform3, GeometryError> {
        if self.is_identity() {
            Ok(AffineTransform3::identity())
        } else {
            crate::morph::rigid_transform(&self, center)
        }
    }
    /// Unequal radii clamp the cubic transition. Native near-equal radii use
    /// unbounded angular rate, measured separately from the radius zero cutoff.
    pub fn angle_at(self, point: Point3) -> Result<Real, GeometryError> {
        if self.is_identity() {
            return Ok(0.);
        }
        let [x, y] = self.radial_coordinates(point);
        self.angle_from_radius(point, x.hypot(y))
    }
    fn radial_coordinates(self, point: Point3) -> [Real; 2] {
        [self.frame.x_axis(), self.frame.y_axis()].map(|a| {
            a.as_vector()
                .dot_point_difference(point, self.frame.origin())
        })
    }
    fn angle_from_radius(self, point: Point3, radius: Real) -> Result<Real, GeometryError> {
        if self.is_identity() {
            return Ok(0.);
        }
        let axes = [self.frame.x_axis(), self.frame.y_axis()];
        if radius == 0. {
            return Ok(0.);
        }
        if self.near_equal {
            if radius.is_finite() {
                let fraction = radius / self.radius0;
                let angle = self.angle * fraction;
                if fraction.is_normal() && angle.is_normal() {
                    return Ok(angle);
                }
                return crate::scaled_quotient(radius, self.angle, self.radius0);
            }
            // Scale before projecting so both a displacement and its radius
            // may exceed binary64 while the resulting angle remains finite.
            let scale = 0.125;
            let local = axes.map(|a| {
                a.as_vector()
                    .scaled_dot_point_difference(point, self.frame.origin(), scale)
            });
            return crate::scaled_quotient(
                local[0].hypot(local[1]),
                self.angle,
                self.radius0 * scale,
            );
        }
        if self.radius0 < self.radius1 {
            if radius <= self.radius0 {
                return Ok(0.);
            }
            if radius >= self.radius1 {
                return Ok(self.angle);
            }
        } else {
            if radius >= self.radius0 {
                return Ok(0.);
            }
            if radius <= self.radius1 {
                return Ok(self.angle);
            }
        }
        let t = (radius - self.radius0) / (self.radius1 - self.radius0);
        let blend = t * t * (3. - 2. * t);
        let angle = self.angle * blend;
        if t.is_normal() && blend.is_normal() && angle.is_normal() {
            return Ok(angle);
        }
        use crate::exact_scalar::{rational, scalar};
        let t = (rational(radius) - rational(self.radius0))
            / (rational(self.radius1) - rational(self.radius0));
        let angle = scalar(&(rational(self.angle) * &t * &t * (rational(3.) - rational(2.) * t)))?;
        if angle == 0. {
            return Err(GeometryError::Degenerate {
                context: "maelstrom angle underflow",
            });
        }
        Ok(angle)
    }
    /// Common axis-aligned planes rotate two world coordinates directly.
    /// This retains raw cardinal residuals without subtracting a large radial
    /// displacement and paying for rational recovery on ordinary quadrants.
    fn cardinal_point(
        self,
        point: Point3,
        sine: Real,
        cosine: Real,
        normal: usize,
        sign: Real,
    ) -> Result<Point3, GeometryError> {
        let axes = [(normal + 1) % 3, (normal + 2) % 3];
        let rows = [[cosine, -sine * sign], [sine * sign, cosine]];
        let origin = self.frame.origin().to_array();
        let mut coordinates = point.to_array();
        let fast: Result<Option<Point3>, GeometryError> = (|| {
            for (axis, row) in axes.into_iter().zip(rows) {
                let mut weights = [0.; 3];
                weights[axes[0]] = row[0];
                weights[axes[1]] = row[1];
                let delta =
                    Vector3::try_from(weights)?.dot_point_difference(point, self.frame.origin());
                let value = origin[axis] + delta;
                if !value.is_finite()
                    || (origin[axis] != 0.
                        && delta != 0.
                        && value.abs() < origin[axis].abs().max(delta.abs()) * 0.125)
                {
                    return Ok(None);
                }
                coordinates[axis] = value;
            }
            Ok(Some(Point3::try_from(coordinates)?))
        })();
        if let Ok(Some(point)) = fast {
            return Ok(point);
        }
        use crate::exact_scalar::{rational, scalar};
        for (axis, row) in axes.into_iter().zip(rows) {
            coordinates[axis] = scalar(
                &(rational(origin[axis])
                    + rational(row[0])
                        * (rational(point.to_array()[axes[0]]) - rational(origin[axes[0]]))
                    + rational(row[1])
                        * (rational(point.to_array()[axes[1]]) - rational(origin[axes[1]]))),
            )?;
        }
        Point3::try_from(coordinates)
    }
    fn exact_point(self, point: Point3, sine: Real, cosine: Real) -> Result<Point3, GeometryError> {
        use crate::exact_scalar::{Rational, rational, scalar};
        let source = point.to_array().map(rational);
        let origin = self.frame.origin().to_array().map(rational);
        let axes = [self.frame.x_axis(), self.frame.y_axis()]
            .map(|a| a.as_vector().to_array().map(rational));
        let local: [Rational; 2] = std::array::from_fn(|i| {
            (0..3)
                .map(|j| &axes[i][j] * (&source[j] - &origin[j]))
                .sum()
        });
        let sine = rational(sine);
        let complement = rational(cosine) - rational(1.);
        let delta = [
            &complement * &local[0] - &sine * &local[1],
            &sine * &local[0] + &complement * &local[1],
        ];
        let mapped: [_; 3] = std::array::from_fn(|i| {
            scalar(&(&source[i] + &axes[0][i] * &delta[0] + &axes[1][i] * &delta[1]))
        });
        let [x, y, z] = mapped;
        Point3::try_new(x?, y?, z?)
    }
}
impl PointMorph for MaelstromPointMorph {
    fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
        if self.is_identity() {
            return Ok(point);
        }
        let local = self.radial_coordinates(point);
        let angle = self.angle_from_radius(point, local[0].hypot(local[1]))?;
        if angle == 0. {
            return Ok(point);
        }
        // The native map keeps raw sin/cos, including tiny and cardinal-angle
        // residuals, rather than the affine command cardinal cleanup policy.
        let (sine, cosine) = angle.sin_cos();
        if let Some((normal, sign)) = self.cardinal_normal {
            return self.cardinal_point(point, sine, cosine, normal, sign);
        }
        let fast: Result<Option<Point3>, GeometryError> = (|| {
            let [x, y] = local;
            let delta = Vector3::try_new(
                (cosine - 1.).mul_add(x, -sine * y),
                sine.mul_add(x, (cosine - 1.) * y),
                0.,
            )?;
            let delta = self.frame.vector_at(delta.to_array())?;
            let mapped = point.translated(delta)?;
            let stable = mapped
                .to_array()
                .into_iter()
                .zip(point.to_array())
                .zip(delta.to_array())
                .all(|((a, p), d)| {
                    d == 0. || (a.is_normal() && a.abs() >= p.abs().max(d.abs()) * 0.125)
                });
            Ok(stable.then_some(mapped))
        })();
        match fast {
            Ok(Some(p)) => Ok(p),
            Ok(None) | Err(_) => self.exact_point(point, sine, cosine),
        }
    }
    fn morph_nurbs_curve(
        &self,
        curve: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<NurbsCurve, GeometryError> {
        if self.is_identity() {
            Ok(curve.clone())
        } else if self.preserve_structure {
            self.morph_nurbs_curve_controls(curve)
        } else {
            crate::morph::fit_curve(self, curve, tolerance)
        }
    }
    fn morph_nurbs_surface(
        &self,
        surface: &NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<NurbsSurface, GeometryError> {
        if self.is_identity() {
            Ok(surface.clone())
        } else if self.preserve_structure {
            self.morph_nurbs_surface_controls(surface)
        } else {
            crate::morph::fit_surface(self, surface, tolerance)
        }
    }
    fn morph_brep(&self, brep: &Brep, tolerance: Tolerance) -> Result<Brep, GeometryError> {
        if self.is_identity() {
            return Ok(brep.clone());
        }
        brep.morphed(
            &self.with_preserve_structure(self.preserve_structure && brep.faces().len() == 1),
            tolerance,
        )
    }
}

#[cfg(test)]
mod tests;
