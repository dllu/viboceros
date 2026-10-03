//! Circular spine bending inferred from public BendSpaceMorph point maps.

use crate::{
    AffineTransform3, Frame3, GeometryError, NurbsCurve, NurbsSurface, Point3, PointMorph, Real,
    Tolerance, require_finite,
};

const SDK_ZERO: Real = 2.3283064365386963e-10;

#[derive(Clone, Copy, Debug)]
pub struct BendPointMorph {
    frame: Frame3,
    radius: Real,
    angle: Real,
    arc_length: Real,
    symmetric: bool,
    non_attenuated: bool,
    preserve_structure: bool,
}

impl BendPointMorph {
    /// Public SDK-style construction. An explicit angle bounds the circular
    /// region while the through point determines radius and plane. With no
    /// angle, `straight` extends a shorter bend region through the spine end.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        start: Point3,
        end: Point3,
        through: Point3,
        angle: Option<Real>,
        straight: bool,
        symmetric: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if let Some(angle) = angle {
            require_finite([angle], "bend angle")?;
            if angle <= 0. || angle > std::f64::consts::TAU {
                return Err(GeometryError::Degenerate {
                    context: "bend angle",
                });
            }
        }
        let spine = start.vector_to(end)?;
        let frame =
            Frame3::try_from_directions(start, spine, start.vector_to(through)?, tolerance)?;
        let [axial, radial, _] = frame.coordinates_of(through)?;
        if radial <= 0. {
            return Err(GeometryError::Degenerate {
                context: "bend through point",
            });
        }
        let distance = axial.hypot(radial);
        let numerator = distance * (distance * 0.5);
        let quotient = numerator / radial;
        // Retain the measured ordinary binary64 construction order. Use the
        // exact quotient only when intermediates lose representable range.
        let radius = if numerator.is_normal() && quotient.is_normal() {
            quotient
        } else {
            crate::exact_scalar::scaled_quotient(distance, distance * 0.5, radial)?
        };
        let angle = if let Some(angle) = angle {
            angle
        } else {
            let through_angle = 2. * radial.atan2(axial);
            if straight {
                through_angle.max(spine.length()? / radius)
            } else {
                through_angle
            }
        };
        // Native captures isolate independent angle-in-degrees and arc-length
        // cutoffs at 2^-32. Keep these SDK construction checks separate from the
        // mathematical arc constructor below.
        if angle.to_degrees() <= SDK_ZERO || radius * angle <= SDK_ZERO {
            return Err(GeometryError::Degenerate {
                context: "bend circular region",
            });
        }
        Self::try_from_arc(frame, radius, angle, symmetric)
    }

    /// Actual Bend command construction. A zero angle restores the through-point
    /// mode. LimitToSpine fixes arc length; for a target beyond the circular region,
    /// its tangent extension passes through the target. This differs from SDK
    /// `straight`, which retains the through-point radius.
    #[allow(clippy::too_many_arguments)]
    pub fn try_for_command(
        start: Point3,
        end: Point3,
        through: Point3,
        angle: Option<Real>,
        limit_to_spine: bool,
        symmetric: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let angle = angle.filter(|angle| *angle != 0.);
        if limit_to_spine && let Some(angle) = angle {
            require_finite([angle], "bend angle")?;
            if angle.to_degrees() <= SDK_ZERO || angle > std::f64::consts::TAU {
                return Err(GeometryError::Degenerate {
                    context: "bend angle",
                });
            }
            let spine = start.vector_to(end)?;
            let length = spine.length()?;
            if length <= SDK_ZERO {
                return Err(GeometryError::Degenerate {
                    context: "bend circular region",
                });
            }
            let frame =
                Frame3::try_from_directions(start, spine, start.vector_to(through)?, tolerance)?;
            // The picked point supplies the plane here, not a candidate radius.
            return Self::try_from_arc(frame, length / angle, angle, symmetric);
        }
        let mut morph = Self::try_new(start, end, through, angle, false, symmetric, tolerance)?;
        if !limit_to_spine {
            return Ok(morph);
        }
        let length = start.distance_to(end)?;
        if morph.arc_length > length {
            let [axial, radial, _] = morph.frame.coordinates_of(through)?;
            // Scale the tangent incidence equation before evaluating it. The
            // half-angle expression avoids cancellation for nearly straight arcs.
            let scale = axial.abs().max(radial).max(length);
            let (a, b, l) = (axial / scale, radial / scale, length / scale);
            let (mut low, mut high) = (0., morph.angle);
            for _ in 0..128 {
                let mid = low + (high - low) * 0.5;
                if mid == low || mid == high {
                    break;
                }
                let (sine, cosine) = mid.sin_cos();
                let residual = b * cosine - a * sine + l * (2. * (mid * 0.5).sin().powi(2)) / mid;
                if residual > 0. {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            morph.angle = low + (high - low) * 0.5;
            morph.radius = length / morph.angle;
        } else {
            morph.angle = length / morph.radius;
        }
        morph.arc_length = length;
        require_finite(
            [morph.radius, morph.angle, morph.arc_length],
            "limited bend arc",
        )?;
        Ok(morph)
    }

    /// The frame's x axis follows the original spine, y points into the bend,
    /// and z is the unchanged binormal. Circular regions have tangent extensions.
    pub fn try_from_arc(
        frame: Frame3,
        radius: Real,
        angle: Real,
        symmetric: bool,
    ) -> Result<Self, GeometryError> {
        let arc_length = radius * angle;
        require_finite([radius, angle, arc_length], "bend arc")?;
        if radius <= 0. || angle <= 0. || arc_length <= 0. {
            return Err(GeometryError::Degenerate {
                context: "bend arc",
            });
        }
        Ok(Self {
            frame,
            radius,
            angle,
            arc_length,
            symmetric,
            non_attenuated: false,
            preserve_structure: false,
        })
    }

    pub fn with_non_attenuated(mut self, value: bool) -> Self {
        self.non_attenuated = value;
        self
    }

    pub fn with_preserve_structure(mut self, value: bool) -> Self {
        self.preserve_structure = value;
        self
    }

    pub const fn radius(self) -> Real {
        self.radius
    }
    pub const fn angle(self) -> Real {
        self.angle
    }
    pub const fn arc_length(self) -> Real {
        self.arc_length
    }

    /// Rigid placement about a bounds center. Uniform bends use the circular
    /// tangent frame; attenuated bends average forward world-axis derivatives
    /// with their dual normals. Both policies are measured in native rigid groups.
    pub fn rigid_transform(self, center: Point3) -> Result<AffineTransform3, GeometryError> {
        if !self.non_attenuated {
            return crate::morph::rigid_transform(&self, center);
        }
        let [axial, _, _] = self.frame.coordinates_of(center)?;
        if axial == 0. || (axial < 0. && !self.symmetric) {
            return Ok(AffineTransform3::identity());
        }
        let angle = (axial.abs() / self.radius).min(self.angle) * axial.signum();
        let rotation =
            AffineTransform3::try_rotation(self.frame.origin(), self.frame.z_axis(), angle)?;
        AffineTransform3::try_mapping_origins(
            rotation.linear_rows(),
            center,
            self.morph_point(center)?,
        )
    }
}

impl PointMorph for BendPointMorph {
    fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
        let [axial, radial, binormal] = self.frame.coordinates_of(point)?;
        if axial == 0. || (axial < 0. && !self.symmetric) {
            return Ok(point);
        }
        let distance = axial.abs();
        let bent_radius = self.radius - radial;
        let phi = if distance >= self.arc_length {
            self.angle
        } else {
            let phi = distance / self.radius;
            if self.non_attenuated || bent_radius <= 0. {
                phi
            } else {
                let t = distance / self.arc_length;
                phi + radial / bent_radius * self.angle * (t * (1. - t) * (1. - 2. * t))
            }
        };
        require_finite([phi, bent_radius], "bend mapped angle")?;
        let (sine, cosine) = phi.sin_cos();
        let mut mapped_radial = self.radius * (2. * (phi * 0.5).sin().powi(2)) + radial * cosine;
        if bent_radius == 0. {
            mapped_radial = self.radius;
        } else if !mapped_radial.is_finite() {
            // Near the center, large opposite terms can exceed binary64 range
            // even though the final circle coordinate is finite.
            mapped_radial = self.radius - bent_radius * cosine;
        }
        let mut mapped_axial = bent_radius * sine;
        if distance > self.arc_length {
            let extension = distance - self.arc_length;
            mapped_radial += extension * sine;
            mapped_axial += extension * cosine;
        }
        self.frame
            .point_at([mapped_axial * axial.signum(), mapped_radial, binormal])
    }

    fn morph_nurbs_curve(
        &self,
        source: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<NurbsCurve, GeometryError> {
        if self.preserve_structure {
            self.morph_nurbs_curve_controls(source)
        } else {
            crate::morph::fit_curve(self, source, tolerance)
        }
    }

    fn morph_nurbs_surface(
        &self,
        source: &NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<NurbsSurface, GeometryError> {
        if self.preserve_structure {
            self.morph_nurbs_surface_controls(source)
        } else {
            crate::morph::fit_surface(self, source, tolerance)
        }
    }
}

#[cfg(test)]
mod tests;
