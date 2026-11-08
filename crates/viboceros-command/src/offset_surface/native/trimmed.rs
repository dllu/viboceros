//! Offset the support surface, retaining authored UV topology and certified images.
use super::*;

pub(super) fn offset(
    source: &Brep,
    options: Options,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    let face = &source.faces()[0];
    if face
        .surface()
        .discontinuity_parameters(viboceros_geometry::CurveContinuity::G2, tolerance.angular())?
        .iter()
        .any(|axis| !axis.is_empty())
    {
        return Err(viboceros_smlib::Error::Kernel(
            "Trimmed offsets across curvature breaks require separate support charts".into(),
        )
        .into());
    }
    let distance = options.distance * if face.is_reversed() { -1. } else { 1. };
    let support = Brep::try_surface_face(face.surface().clone(), tolerance)?;
    let requested = options.effective_tolerance(tolerance);
    let surface = Shape::from_brep(&support, tolerance)?
        .offset_with_accuracy(distance, false, requested * 0.25)?
        .to_single_surface_approximated(requested * 0.25)?;
    // The kernel may extend a fitted support slightly beyond its input chart.
    // Restrict it exactly; reparameterizing would move the authored UV loops.
    let surface = surface.try_trimmed(face.surface().domain_u(), face.surface().domain_v())?;
    Ok(Geometry::Brep(
        source.try_with_edited_single_surface(surface, tolerance)?,
    ))
}
