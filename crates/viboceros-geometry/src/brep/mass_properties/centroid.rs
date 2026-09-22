use super::*;
use crate::{AreaMassProperties, area_mass_properties::surface};

impl Brep {
    /// Area-weighted centroid of all retained faces, including exact trim
    /// boundaries and holes. Face reversal does not change area mass.
    pub fn area_mass_properties(
        &self,
        tolerance: Tolerance,
    ) -> Result<AreaMassProperties, GeometryError> {
        let mut result = AreaMassProperties::default();
        for face in &self.faces {
            let local = face.local_parameter_frame()?;
            let face = local.face.as_ref();
            let mut frame = surface::Frame::new(&face.surface, tolerance)?;
            let mut rectangular = face_covers_full_surface_domain(face, tolerance)?;
            if !rectangular && let Some(bounds) = rectangular_face_trim_bounds(face, tolerance)? {
                frame.surface = frame
                    .surface
                    .try_trimmed(bounds[0][0]..=bounds[0][1], bounds[1][0]..=bounds[1][1])?;
                rectangular = true;
            }
            let integrals = if rectangular {
                surface::rectangle(&frame.surface, frame.absolute, tolerance.relative())?
            } else {
                let mut values = [0.; 4];
                for (component, value) in values.iter_mut().enumerate() {
                    *value = trimmed::integrate_density(
                        face,
                        &frame.surface,
                        frame.absolute,
                        tolerance.relative(),
                        |point, normal, sign| {
                            Ok(surface::density(
                                point,
                                sign * 4. * normal.length()?,
                                component,
                            ))
                        },
                    )?;
                }
                values
            };
            result.add(&frame.finish(integrals)?);
        }
        result.centroid()?;
        Ok(result)
    }
}
