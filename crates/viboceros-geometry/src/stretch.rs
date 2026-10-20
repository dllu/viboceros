//! Axial stretch with a clamped cubic transition between two planes.
use crate::{
    AffineTransform3, Brep, GeometryError, MorphInterpolationSolver, NurbsCurve, NurbsSurface,
    Point3, PointMorph, Real, Tolerance, UnitVector3, require_finite,
};

#[derive(Clone, Copy, Debug)]
pub struct StretchPointMorph {
    start: Point3,
    direction: UnitVector3,
    length: Real,
    new_length: Real,
    requested_length: Real,
    preserve_structure: bool,
    interpolation_solver: Option<MorphInterpolationSolver>,
}

impl StretchPointMorph {
    /// Public SDK length semantics: negative new lengths collapse the end
    /// plane onto the start plane. Points behind the start plane remain fixed;
    /// points beyond the end plane receive a constant axial translation.
    pub fn try_new(start: Point3, end: Point3, new_length: Real) -> Result<Self, GeometryError> {
        require_finite([new_length], "stretch length")?;
        let length = start.distance_to(end)?;
        if length <= 2.3283064365386963e-10 {
            return Err(GeometryError::Degenerate {
                context: "stretch axis",
            });
        }
        let direction = start.direction_to(end)?;
        Ok(Self {
            start,
            direction,
            length,
            new_length: new_length.max(0.),
            requested_length: new_length,
            preserve_structure: false,
            interpolation_solver: None,
        })
    }
    /// The target contributes its signed projection onto the original axis;
    /// perpendicular offsets do not turn or shear that axis.
    pub fn try_with_target(
        start: Point3,
        end: Point3,
        target: Point3,
    ) -> Result<Self, GeometryError> {
        let direction = start.direction_to(end)?;
        Self::try_new(
            start,
            end,
            direction.as_vector().dot_point_difference(target, start),
        )
    }
    pub fn with_preserve_structure(mut self, preserve: bool) -> Self {
        self.preserve_structure = preserve;
        self
    }
    pub fn with_interpolation_solver(mut self, solver: MorphInterpolationSolver) -> Self {
        self.interpolation_solver = Some(solver);
        self
    }
    pub fn is_identity(self) -> bool {
        self.length == self.new_length
    }
    pub fn length(self) -> Real {
        self.length
    }
    pub fn requested_length(self) -> Real {
        self.requested_length
    }
    pub fn new_length(self) -> Real {
        self.new_length
    }
    pub(crate) fn axis(self) -> (Point3, UnitVector3) {
        (self.start, self.direction)
    }
    pub fn rigid_transform(self, center: Point3) -> Result<AffineTransform3, GeometryError> {
        if self.is_identity() {
            Ok(AffineTransform3::identity())
        } else {
            crate::morph::rigid_transform(&self, center)
        }
    }
    fn exact_point(self, point: Point3) -> Result<Point3, GeometryError> {
        use crate::exact_scalar::{Rational, rational, scalar};
        let source = point.to_array().map(rational);
        let start = self.start.to_array().map(rational);
        let axis = self.direction.as_vector().to_array().map(rational);
        let t: Rational = (0..3)
            .map(|i| (&source[i] - &start[i]) * &axis[i])
            .sum::<Rational>()
            / rational(self.length);
        let zero = rational(0.);
        let one = rational(1.);
        let blend = if t <= zero {
            zero
        } else if t >= one {
            one
        } else {
            &t * &t * (rational(3.) - rational(2.) * &t)
        };
        let amount = blend * (rational(self.new_length) - rational(self.length));
        let [x, y, z] = std::array::from_fn(|i| scalar(&(&source[i] + &axis[i] * &amount)));
        Point3::try_new(x?, y?, z?)
    }
}
impl PointMorph for StretchPointMorph {
    fn rebuild_grip_curve(
        &self,
        curve: NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<crate::Curve3, GeometryError> {
        curve.into_curve_with_derivative_breaks(tolerance)
    }

    fn interpolation_solver(&self) -> Option<MorphInterpolationSolver> {
        self.interpolation_solver
    }
    fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
        if self.is_identity() {
            return Ok(point);
        }
        let axis = self.direction.as_vector();
        let axial = axis.dot_point_difference(point, self.start);
        if axial <= 0. {
            return Ok(point);
        }
        let t = (axial / self.length).min(1.);
        let blend = t * t * (3. - 2. * t);
        let amount = (self.new_length - self.length) * blend;
        if !blend.is_normal() || !amount.is_normal() {
            return self.exact_point(point);
        }
        let delta = axis.scaled(amount)?;
        let mapped = point.translated(delta)?;
        let stable = mapped
            .to_array()
            .into_iter()
            .zip(point.to_array())
            .zip(delta.to_array())
            .all(|((a, p), d)| d == 0. || a.is_normal() && a.abs() >= p.abs().max(d.abs()) * 0.125);
        if stable {
            Ok(mapped)
        } else {
            self.exact_point(point)
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
            Ok(brep.clone())
        } else {
            if !self.preserve_structure
                && let Ok(Some(result)) = brep.try_stretch_affine_faces(self, tolerance)
            {
                return Ok(result);
            }
            brep.morphed(
                &self.with_preserve_structure(self.preserve_structure && brep.faces().len() == 1),
                tolerance,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(z: Real) -> Point3 {
        Point3::try_new(2., -3., z).unwrap()
    }
    #[test]
    fn cubic_transition_pins_the_start_and_translates_the_far_side() {
        let m = StretchPointMorph::try_new(
            Point3::try_from([0.; 3]).unwrap(),
            Point3::try_new(0., 0., 10.).unwrap(),
            20.,
        )
        .unwrap();
        for (z, expected) in [
            (-2., -2.),
            (0., 0.),
            (2.5, 4.0625),
            (5., 10.),
            (7.5, 15.9375),
            (10., 20.),
            (12., 22.),
        ] {
            assert_eq!(m.morph_point(p(z)).unwrap(), p(expected));
        }
        let identity = StretchPointMorph::try_new(p(0.), p(10.), 10.).unwrap();
        assert_eq!(identity.morph_point(p(5.)).unwrap(), p(5.));
        assert_eq!(
            identity.rigid_transform(p(5.)).unwrap(),
            AffineTransform3::identity()
        );
    }
    #[test]
    fn perpendicular_target_components_do_not_turn_the_axis_and_negative_lengths_collapse() {
        let a = Point3::try_from([0.; 3]).unwrap();
        let b = Point3::try_new(0., 0., 10.).unwrap();
        let m = StretchPointMorph::try_with_target(a, b, Point3::try_new(8., -9., 20.).unwrap())
            .unwrap();
        assert_eq!(m.morph_point(p(5.)).unwrap(), p(10.));
        let collapse = StretchPointMorph::try_new(a, b, -20.).unwrap();
        assert_eq!(collapse.new_length(), 0.);
        assert_eq!(collapse.requested_length(), -20.);
        assert_eq!(collapse.morph_point(p(2.5)).unwrap(), p(0.9375));
        assert_eq!(collapse.morph_point(p(7.5)).unwrap(), p(-0.9375));
        assert_eq!(collapse.morph_point(p(10.)).unwrap(), p(0.));
    }
    #[test]
    fn underflowed_blends_recover_finite_significant_displacements() {
        let m = StretchPointMorph::try_new(
            Point3::try_from([0.; 3]).unwrap(),
            Point3::try_new(0., 0., 1.).unwrap(),
            1e300,
        )
        .unwrap();
        let q = m
            .morph_point(Point3::try_new(0., 0., 1e-200).unwrap())
            .unwrap();
        assert!((q.z() / 3e-100 - 1.).abs() < 1e-14);
        assert_eq!(q.x(), 0.);
        assert_eq!(q.y(), 0.);
        let m = StretchPointMorph::try_new(p(0.), p(10.), 0.01).unwrap();
        assert_eq!(m.morph_point(p(10.)).unwrap(), p(0.01));
    }
}
