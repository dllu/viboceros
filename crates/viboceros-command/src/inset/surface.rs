//! Surface admission and staging shared by typed and interactive Inset.
use super::*;
pub(super) fn is_surface(geometry: &Geometry) -> bool {
    matches!(geometry, Geometry::Brep(_) | Geometry::NurbsSurface(_))
}
pub(super) fn require_polygon_options(options: Options) -> Result<(), CommandError> {
    if options.crease_corners {
        Err(CommandError::Usage(
            "CreaseCorners requires planar surface faces",
        ))
    } else {
        Ok(())
    }
}
pub(super) fn prepare(
    geometry: &Geometry,
    faces: &[usize],
    options: Options,
    tolerance: Tolerance,
) -> Result<(Geometry, ComponentSelectionKind, usize), CommandError> {
    let source;
    let brep = match geometry {
        Geometry::Brep(b) => b,
        Geometry::NurbsSurface(s) => {
            source = Brep::try_surface_face(s.clone(), tolerance)?;
            &source
        }
        _ => return Err(CommandError::UnsupportedInsetGeometry),
    };
    let result =
        brep.try_inset_planar_faces(faces, options.distance, options.crease_corners, tolerance)?;
    let mut bytes = result.vertices().len().saturating_mul(32);
    for edge in result.edges() {
        bytes = bytes
            .saturating_add(128)
            .saturating_add(edge.curve().control_points().len().saturating_mul(32))
            .saturating_add(edge.curve().knots().len().saturating_mul(8));
    }
    for face in result.faces() {
        bytes = bytes
            .saturating_add(128)
            .saturating_add(face.surface().control_points().len().saturating_mul(32))
            .saturating_add(face.surface().knots_u().len().saturating_mul(8))
            .saturating_add(face.surface().knots_v().len().saturating_mul(8));
        for trim in face.loops().iter().flat_map(|l| l.trims()) {
            bytes = bytes
                .saturating_add(128)
                .saturating_add(trim.curve().control_points().len().saturating_mul(24))
                .saturating_add(trim.curve().knots().len().saturating_mul(8));
        }
    }
    Ok((
        Geometry::Brep(result),
        ComponentSelectionKind::BrepFace,
        bytes,
    ))
}
