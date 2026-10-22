//! Mesh topology conversion and optional vertex-limit interpolation.
use super::*;
use crate::Tolerance;

/// Mesh vertices define the control net unless interpolation is requested.
/// Unwelded edges and boundary corners can independently become hard features.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubDFromMeshOptions {
    pub interpolate: bool,
    pub corners: bool,
    pub creases: bool,
}
impl SubD {
    /// Reuses OpenNURBS' mesh conversion, including coincident raw seams,
    /// n-gon overlays, collinear boundary merging and nonmanifold cleanup.
    /// Interpolated output uses the shared sparse limit-position solver.
    /// The source stays unchanged if topology conversion or fitting fails.
    pub fn try_from_mesh(
        mesh: &TriangleMesh,
        options: SubDFromMeshOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let converted = Self::from_native(viboceros_opennurbs::subd_from_mesh(
            &mesh.open_nurbs_data(),
            options.corners,
            options.creases,
        )?)?;
        if options.interpolate {
            converted.try_with_limit_points(converted.control_points(), tolerance)
        } else {
            Ok(converted)
        }
    }
}
