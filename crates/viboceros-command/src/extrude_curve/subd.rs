//! SubD profile preparation shares exact extrusion and sampled surface fitting.
use super::*;
use viboceros_geometry::{CurveRef, SubD, SubDFromSurfaceMethod};

pub(super) fn extrude(
    curve: &NurbsCurve,
    start: Vector3,
    end: Vector3,
    tolerance: Tolerance,
) -> Result<SubD, CommandError> {
    let typed = Brep::try_command_curve_extrusion(curve, start, end, false, tolerance)?;
    let surface = if let Some(extrusion) = typed.as_ref().and_then(Brep::extrusion) {
        extrusion.nurbs_form()?.surface
    } else {
        // The public oriented support plane determines which side of an
        // oblique planar extrusion faces out, including open curved profiles.
        let path = Vector3::try_from(std::array::from_fn(|i| {
            end.to_array()[i] - start.to_array()[i]
        }))?;
        let normal = if curve.is_linear(tolerance)? {
            None
        } else {
            CurveRef::NurbsCurve(curve).supporting_plane_normal(tolerance)?
        };
        let reverse = normal
            .map(|n| n.as_vector().dot(path))
            .transpose()?
            .is_some_and(|dot| dot < 0.);
        let profile = if reverse {
            curve.reversed()?
        } else {
            curve.clone()
        };
        NurbsSurface::try_extruded_curve(&profile, start, end)?
    };
    Ok(SubD::try_from_surface(
        &surface,
        SubDFromSurfaceMethod::Location,
        true,
        tolerance,
    )?)
}
