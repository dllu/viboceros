//! A typed native extrusion plus its validated canonical B-rep view.
use super::*;
use std::sync::Arc;

/// Immutable extrusion data retained alongside its exact B-rep view. Only
/// constructors that decode both views together can attach this representation.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtrusionRepresentation {
    archive: Arc<[u8]>,
}
impl ExtrusionRepresentation {
    pub fn archive(&self) -> &[u8] {
        &self.archive
    }
    /// Complete profile NURBS form, including the native profile joining policy.
    pub fn nurbs_form(&self) -> Result<ExtrusionNurbsForm, GeometryError> {
        let data = viboceros_opennurbs::extrusion_surface(self.archive())?;
        let surface =
            crate::interop::decode_surface(&data.surface).map_err(|error| match error {
                crate::interop::GeometryCodecError::Geometry(e) => e,
                _ => viboceros_opennurbs::ExtrusionError("Invalid NURBS surface transfer".into())
                    .into(),
            })?;
        Ok(ExtrusionNurbsForm {
            surface,
            caps: data.caps,
            path_axis: data.path_axis,
        })
    }
}
pub struct ExtrusionNurbsForm {
    pub surface: NurbsSurface,
    pub caps: [bool; 2],
    pub path_axis: usize,
}
impl Brep {
    pub fn extrusion(&self) -> Option<&ExtrusionRepresentation> {
        self.extrusion.as_ref()
    }
    pub fn is_extrusion(&self) -> bool {
        self.extrusion.is_some()
    }
    /// Returns an independent explicit B-rep, removing its extrusion representation.
    pub fn explicit_brep(&self) -> Self {
        let mut result = self.clone();
        result.extrusion = None;
        result
    }
    pub fn try_from_extrusion_archive(
        bytes: &[u8],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::from_extrusion_data(viboceros_opennurbs::extrusion_read(bytes)?, tolerance)
    }
    /// Create the public extrusion form of a planar profile, projected onto
    /// the given frame, with optional caps on a closed profile.
    pub fn try_planar_profile_extrusion(
        profile: &NurbsCurve,
        frame: Frame3,
        height: Real,
        cap: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let controls = profile
            .control_points()
            .iter()
            .map(|p| {
                let [x, y, z] = p.point().to_array();
                [x, y, z, p.weight()]
            })
            .collect::<Vec<_>>();
        let mut plane = [0.; 9];
        plane[..3].copy_from_slice(&frame.origin().to_array());
        plane[3..6].copy_from_slice(&frame.x_axis().as_vector().to_array());
        plane[6..].copy_from_slice(&frame.y_axis().as_vector().to_array());
        Self::from_extrusion_data(
            viboceros_opennurbs::extrusion_create(
                viboceros_opennurbs::ExtrusionProfile {
                    degree: profile.degree() as u32,
                    controls: &controls,
                    knots: profile.knots(),
                },
                plane,
                height,
                cap,
            )?,
            tolerance,
        )
    }
    pub(super) fn from_extrusion_data(
        data: viboceros_opennurbs::ExtrusionData,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mut brep =
            crate::interop::decode_brep(&data.brep, tolerance).map_err(|error| match error {
                crate::interop::GeometryCodecError::Geometry(e) => e,
                _ => viboceros_opennurbs::ExtrusionError("Invalid canonical B-rep transfer".into())
                    .into(),
            })?;
        brep.extrusion = Some(ExtrusionRepresentation {
            archive: data.archive.into(),
        });
        Ok(brep)
    }
}
