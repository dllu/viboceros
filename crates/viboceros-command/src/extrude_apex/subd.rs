//! Singular apex fitting and native degree-one B-rep fallback.
use super::*;
use viboceros_geometry::{SubD, SubDFromSurfaceMethod};

pub(super) fn extrude(
    curve: &NurbsCurve,
    apex: Point3,
    crease_splitting: bool,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    let surface = NurbsSurface::try_extruded_curve_to_point(curve, apex)?;
    if curve.degree() == 1 {
        let brep = Brep::try_surface_face_with_native_edge_parameters(surface, tolerance)?;
        let split = if crease_splitting {
            brep.try_split_kinky_faces(0.1_f64.to_radians(), tolerance)?
        } else {
            None
        };
        Ok(Geometry::Brep(split.unwrap_or(brep)))
    } else {
        Ok(Geometry::SubD(SubD::try_from_surface(
            &surface,
            SubDFromSurfaceMethod::Location,
            true,
            tolerance,
        )?))
    }
}
