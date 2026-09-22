use super::*;
use crate::{
    NurbsSurface, Tolerance,
    mass_integration::{SpatialFrame, rectangle_density},
};

pub(crate) struct Frame {
    spatial: SpatialFrame,
    pub(crate) surface: NurbsSurface,
    pub(crate) absolute: Real,
}

impl Frame {
    pub(crate) fn new(surface: &NurbsSurface, tolerance: Tolerance) -> Result<Self, GeometryError> {
        let spatial = SpatialFrame::new(surface.control_point_bounds())?;
        let normalized = spatial.surface(surface)?;
        let absolute = spatial.tolerance(tolerance);
        Ok(Self {
            spatial,
            surface: normalized,
            absolute,
        })
    }

    pub(crate) fn finish(&self, values: [Real; 4]) -> Result<AreaMassProperties, GeometryError> {
        AreaMassProperties::from_local_integrals(self.spatial.origin, self.spatial.scale, values)
    }
}

pub(crate) fn density(point: Point3, area: Real, component: usize) -> Real {
    if component == 0 {
        area
    } else {
        area * point.to_array()[component - 1]
    }
}

pub(crate) fn rectangle(
    surface: &NurbsSurface,
    absolute: Real,
    relative: Real,
) -> Result<[Real; 4], GeometryError> {
    rectangle_density(surface, absolute, relative, |p, n, component| {
        Ok(density(p, n.length()? * 4., component))
    })
}

impl NurbsSurface {
    /// Area-weighted centroid of the complete natural surface, obtained from
    /// its rational derivatives rather than a display mesh/control net.
    pub fn area_mass_properties(
        &self,
        tolerance: Tolerance,
    ) -> Result<AreaMassProperties, GeometryError> {
        let local = self.local_parameter_frame(std::iter::empty())?;
        let frame = Frame::new(&local.surface, tolerance)?;
        frame.finish(rectangle(
            &frame.surface,
            frame.absolute,
            tolerance.relative(),
        )?)
    }
}
