//! Surface admission and staging shared by typed and interactive Inset.
use super::*;
pub(super) fn is_surface(geometry: &Geometry) -> bool {
    matches!(geometry, Geometry::Brep(_) | Geometry::NurbsSurface(_))
}
pub(super) fn require_polygon_options(options: Options) -> Result<(), CommandError> {
    if options.crease_corners || options.loose {
        Err(CommandError::Usage(
            "CreaseCorners and Loose require planar surface faces",
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
    let result = brep.try_inset_planar_faces_with_options(
        faces,
        viboceros_geometry::PlanarInsetOptions {
            distance: options.distance,
            crease_corners: options.crease_corners,
            loose: options.loose,
        },
        tolerance,
        viboceros_geometry::PlanarInsetOperations {
            curves: offset_curve::offset_operations(),
            edit_points: edit_point_solver(),
        },
    )?;
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

fn edit_point_solver() -> Option<viboceros_geometry::CurveEditPointSolver> {
    #[cfg(feature = "native-smlib")]
    {
        Some(viboceros_smlib::curve_edit_point_interpolation)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        None
    }
}
