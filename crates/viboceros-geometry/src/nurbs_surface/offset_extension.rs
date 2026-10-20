//! End-span continuation with the offset solver's exact seam classification.
use super::*;

impl NurbsSurface {
    pub(crate) fn extended_for_offset(
        &self,
        intervals: [Option<RangeInclusive<Real>>; 2],
    ) -> Result<Self, GeometryError> {
        let mut surface = self.clone();
        for (axis, interval) in intervals.into_iter().enumerate() {
            let Some(interval) = interval else { continue };
            if crate::surface_geodesic::chart_closes(&surface, axis)? {
                return Err(GeometryError::SurfaceExtensionDirectionMustBeOpen {
                    direction: if axis == 0 { "U" } else { "V" },
                });
            }
            let domain = if axis == 0 {
                surface.domain_u()
            } else {
                surface.domain_v()
            };
            validate_surface_extension_interval(&interval, domain)?;
            surface = if axis == 0 {
                surface
                    .map_u_control_curves(|c| c.try_extended_control_curve_to(interval.clone()))?
            } else {
                surface
                    .map_v_control_curves(|c| c.try_extended_control_curve_to(interval.clone()))?
            };
        }
        Ok(surface)
    }
}
