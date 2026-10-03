//! Radial and one-directional taper maps inferred from public point witnesses.
use crate::{
    AffineTransform3, Brep, Frame3, GeometryError, NurbsCurve, NurbsSurface, Point3, PointMorph,
    Real, Tolerance, require_finite,
};

const SDK_ZERO: Real = 2.3283064365386963e-10;

#[derive(Clone, Copy, Debug)]
pub struct TaperPointMorph {
    frame: Frame3,
    length: Real,
    start_radius: Real,
    end_radius: Real,
    radius_change: Real,
    flat: bool,
    infinite: bool,
    preserve_structure: bool,
}

impl TaperPointMorph {
    /// Public SDK construction. Flat mode follows the deterministic plane X
    /// direction perpendicular to the axis; model fitting tolerance is separate.
    pub fn try_new(
        start: Point3,
        end: Point3,
        start_radius: Real,
        end_radius: Real,
        flat: bool,
        infinite: bool,
    ) -> Result<Self, GeometryError> {
        require_finite([start_radius, end_radius], "taper radii")?;
        let axis = start.vector_to(end)?;
        let length = axis.length()?;
        if length <= SDK_ZERO || start_radius <= SDK_ZERO || end_radius <= SDK_ZERO {
            return Err(GeometryError::Degenerate {
                context: "taper SDK definition",
            });
        }
        let frame = Frame3::try_from_normal(start, axis, Tolerance::NUMERICAL_VALIDATION)?;
        Self::try_from_frame(frame, length, start_radius, end_radius, flat, infinite)
    }

    /// Mathematical construction with an explicit flat direction and positive
    /// lengths/radii. This accepts scales below the public SDK validity cutoff.
    pub fn try_from_frame(
        frame: Frame3,
        length: Real,
        start_radius: Real,
        end_radius: Real,
        flat: bool,
        infinite: bool,
    ) -> Result<Self, GeometryError> {
        require_finite([length, start_radius, end_radius], "taper definition")?;
        if length <= 0. || start_radius <= 0. || end_radius <= 0. {
            return Err(GeometryError::Degenerate {
                context: "taper definition",
            });
        }
        Ok(Self::from_validated_frame(
            frame,
            length,
            start_radius,
            end_radius,
            flat,
            infinite,
        ))
    }

    /// Actual commands accept signed distances but reject values within the
    /// native zero cutoff. SDK construction above retains its positive rule.
    pub fn try_for_command_frame(
        frame: Frame3,
        length: Real,
        start_radius: Real,
        end_radius: Real,
        flat: bool,
        infinite: bool,
    ) -> Result<Self, GeometryError> {
        require_finite(
            [length, start_radius, end_radius],
            "taper command definition",
        )?;
        if length <= SDK_ZERO || start_radius.abs() <= SDK_ZERO || end_radius.abs() <= SDK_ZERO {
            return Err(GeometryError::Degenerate {
                context: "taper command definition",
            });
        }
        Ok(Self::from_validated_frame(
            frame,
            length,
            start_radius,
            end_radius,
            flat,
            infinite,
        ))
    }

    fn from_validated_frame(
        frame: Frame3,
        length: Real,
        start_radius: Real,
        end_radius: Real,
        flat: bool,
        infinite: bool,
    ) -> Self {
        Self {
            frame,
            length,
            start_radius,
            end_radius,
            radius_change: (end_radius - start_radius) / start_radius,
            flat,
            infinite,
            preserve_structure: false,
        }
    }

    pub fn is_identity(self) -> bool {
        self.start_radius == self.end_radius
    }

    pub fn with_preserve_structure(mut self, preserve: bool) -> Self {
        self.preserve_structure = preserve;
        self
    }

    pub fn rigid_transform(self, center: Point3) -> Result<AffineTransform3, GeometryError> {
        if self.is_identity() {
            Ok(AffineTransform3::identity())
        } else {
            crate::morph::rigid_transform(&self, center)
        }
    }

    fn blend(self, axial: Real) -> Real {
        if self.infinite {
            axial / self.length
        } else if axial <= 0. {
            0.
        } else if axial >= self.length {
            1.
        } else {
            let t = axial / self.length;
            t * t * (3. - 2. * t)
        }
    }

    fn exact_point(self, point: Point3) -> Result<Point3, GeometryError> {
        use crate::exact_scalar::{Rational, rational, scalar};
        let source = point.to_array().map(rational);
        let origin = self.frame.origin().to_array().map(rational);
        let difference: [_; 3] = std::array::from_fn(|i| &source[i] - &origin[i]);
        let axes = self
            .frame
            .axes()
            .map(|a| a.as_vector().to_array().map(rational));
        let local: [Rational; 3] =
            std::array::from_fn(|i| (0..3).map(|j| &axes[i][j] * &difference[j]).sum());
        let t = &local[2] / rational(self.length);
        let one = rational(1.);
        let zero = rational(0.);
        let blend = if self.infinite {
            t
        } else if t <= zero {
            zero
        } else if t >= one {
            one.clone()
        } else {
            &t * &t * (rational(3.) - rational(2.) * &t)
        };
        let change = blend * (rational(self.end_radius) / rational(self.start_radius) - one);
        let mapped: [_; 3] = std::array::from_fn(|i| {
            let radial = &axes[0][i] * &local[0]
                + if self.flat {
                    rational(0.)
                } else {
                    &axes[1][i] * &local[1]
                };
            scalar(&(&source[i] + &change * radial))
        });
        let [x, y, z] = mapped;
        Point3::try_new(x?, y?, z?)
    }
}

impl PointMorph for TaperPointMorph {
    fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
        if self.is_identity() {
            return Ok(point);
        }
        let normal = self.frame.z_axis().as_vector();
        let axial = normal.dot_point_difference(point, self.frame.origin());
        if !self.infinite && axial < 0. {
            return Ok(point);
        }
        // A cardinal-axis zero is exact. Other rounded zero projections and
        // tiny blend factors may become significant after a very large ratio.
        let cardinal = normal.to_array().into_iter().filter(|v| *v != 0.).count() == 1;
        if axial == 0. && cardinal {
            return Ok(point);
        }
        let blend = self.blend(axial);
        if !blend.is_normal() {
            return self.exact_point(point);
        }
        let change = self.radius_change * blend;
        if !change.is_normal() {
            return self.exact_point(point);
        }
        let fast: Result<Option<Point3>, GeometryError> = (|| {
            let [x, y] = self.frame.projected_coordinates_of(point)?;
            let radial = [x * change, if self.flat { 0. } else { y * change }, 0.];
            if (x != 0. && !radial[0].is_normal())
                || (!self.flat && y != 0. && !radial[1].is_normal())
            {
                return Ok(None);
            }
            let delta = self.frame.vector_at(radial)?;
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
            Ok(Some(point)) => Ok(point),
            Ok(None) | Err(_) => self.exact_point(point),
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
