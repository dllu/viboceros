use super::*;
use crate::{
    FiniteSum, Point3, SurfaceVolumeMoments, VolumeMassProperties,
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
        self.volume_flux_with_moments(base, tolerance, SurfaceVolumeMoments::Cone)
    }

    /// Volume flux with an explicit surface first-moment convention. Different
    /// conventions must not be mixed when interpreting unjoined boundary
    /// pieces as one physical distribution. Closed-surface results agree.
    pub fn volume_flux_with_moments(
        &self,
        base: Point3,
        tolerance: Tolerance,
        moments: SurfaceVolumeMoments,
    ) -> Result<VolumeMassProperties, GeometryError> {
        self.volume_integrals::<true>(base, tolerance, moments)
    }

    pub(crate) fn volume_integrals<const FIRST: bool>(
        &self,
        base: Point3,
        tolerance: Tolerance,
        moments: SurfaceVolumeMoments,
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
                let orientation = if face.reversed { -1. } else { 1. };
                let flux = Vector3::try_from(point.to_array())?.dot(normal)? * orientation;
                // div(p)=3; div(p_j*p)=4*p_j. Normal contains 1/4
                // of the span-scaled Jacobian, including UV boundary sign.
                Ok(if component == 0 {
                    flux * (4. / 3.)
                } else {
                    let axis = component - 1;
                    let q = point.to_array()[axis];
                    match moments {
                        SurfaceVolumeMoments::Cone => flux * q,
                        // Three separate coordinate primitives each have
                        // divergence q_j. Their average has diagonal term
                        // q_j^2*n_j/6 and off-diagonal terms q_j*q_i*n_i/3.
                        SurfaceVolumeMoments::CoordinatePrimitives => {
                            q * (flux - 0.5 * q * normal.to_array()[axis] * orientation) * (4. / 3.)
                        }
                    }
                })
            };
            let values = if rectangular {
                if FIRST {
                    rectangle_density(&surface, absolute, tolerance.relative(), density)?
                } else {
                    let [volume] =
                        rectangle_density(&surface, absolute, tolerance.relative(), density)?;
                    [volume, 0., 0., 0.]
                }
            } else {
                let mut values = [0.; 4];
                for (component, value) in
                    values
                        .iter_mut()
                        .enumerate()
                        .take(if FIRST { 4 } else { 1 })
                {
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
    /// Surface flux using the explicitly selected first-moment convention.
    pub fn volume_flux_with_moments(
        &self,
        base: Point3,
        tolerance: Tolerance,
        moments: SurfaceVolumeMoments,
    ) -> Result<VolumeMassProperties, GeometryError> {
        Brep::try_surface_face(self.clone(), tolerance)?
            .volume_flux_with_moments(base, tolerance, moments)
    }

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
