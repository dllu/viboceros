use super::*;
use crate::{
    FiniteSum, Point3, VolumeMassProperties,
    mass_integration::{SpatialFrame, rectangle_density},
};
#[cfg(test)]
mod tests;

impl Brep {
    /// Signed enclosed volume and first moments from exact NURBS/trim geometry.
    /// All faces share one spatial frame: separately recentering open face fluxes
    /// would destroy the divergence-theorem cancellations.
    pub fn volume_mass_properties(
        &self,
        tolerance: Tolerance,
    ) -> Result<VolumeMassProperties, GeometryError> {
        if !self.is_solid() {
            return Err(GeometryError::OpenBrepVolume);
        }
        self.volume_flux(self.bounds().center()?, tolerance)
    }

    /// Oriented cone-flux integrals, including open boundary pieces. All pieces
    /// of an enclosing collection must use the same base point; no independent
    /// recentering of open faces is permitted. An isolated open piece does not
    /// define an enclosed solid and its result depends on the chosen base.
    pub fn volume_flux(
        &self,
        base: Point3,
        tolerance: Tolerance,
    ) -> Result<VolumeMassProperties, GeometryError> {
        let spatial = SpatialFrame::with_origin(self.bounds(), base)?;
        let absolute =
            (spatial.tolerance(tolerance) / self.faces.len() as Real).max(Real::MIN_POSITIVE);
        let mut totals: [FiniteSum; 4] = std::array::from_fn(|_| FiniteSum::default());
        for face in &self.faces {
            let local = face.local_parameter_frame()?;
            let face = local.face.as_ref();
            let mut surface = spatial.surface(&face.surface)?;
            let mut rectangular = face_covers_full_surface_domain(face, tolerance)?;
            if !rectangular && let Some(bounds) = rectangular_face_trim_bounds(face, tolerance)? {
                surface = surface
                    .try_trimmed(bounds[0][0]..=bounds[0][1], bounds[1][0]..=bounds[1][1])?;
                rectangular = true;
            }
            let density = |point: Point3, normal: Vector3, component: usize| {
                let flux = Vector3::try_from(point.to_array())?.dot(normal)?
                    * if face.reversed { -1. } else { 1. };
                // div(p)=3; div(p_j*p)=4*p_j. Normal contains 1/4
                // of the span-scaled Jacobian, including UV boundary sign.
                Ok(if component == 0 {
                    flux * (4. / 3.)
                } else {
                    flux * point.to_array()[component - 1]
                })
            };
            let values = if rectangular {
                rectangle_density(&surface, absolute, tolerance.relative(), density)?
            } else {
                let mut values = [0.; 4];
                for (component, value) in values.iter_mut().enumerate() {
                    *value = trimmed::integrate_density(
                        face,
                        &surface,
                        absolute,
                        tolerance.relative(),
                        |point, normal, _| density(point, normal, component),
                    )?;
                }
                values
            };
            for (sum, value) in totals.iter_mut().zip(values) {
                sum.add(value)?;
            }
        }
        VolumeMassProperties::from_local_integrals(
            spatial.origin,
            spatial.scale,
            [
                totals[0].total()?,
                totals[1].total()?,
                totals[2].total()?,
                totals[3].total()?,
            ],
        )
    }
}

impl NurbsSurface {
    /// Signed cone flux of an open or closed natural surface boundary.
    pub fn volume_flux(
        &self,
        base: Point3,
        tolerance: Tolerance,
    ) -> Result<VolumeMassProperties, GeometryError> {
        Brep::try_surface_face(self.clone(), tolerance)?.volume_flux(base, tolerance)
    }

    /// Mass properties of a closed surface, with periodic seams and singular
    /// boundaries validated by the exact rectangular-face topology builder.
    pub fn volume_mass_properties(
        &self,
        tolerance: Tolerance,
    ) -> Result<VolumeMassProperties, GeometryError> {
        Brep::try_surface_face(self.clone(), tolerance)?.volume_mass_properties(tolerance)
    }
}
