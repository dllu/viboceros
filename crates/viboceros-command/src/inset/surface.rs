//! Surface admission and staging shared by typed and interactive Inset.
use super::*;
pub(super) fn is_surface(geometry: &Geometry) -> bool {
    matches!(geometry, Geometry::Brep(_) | Geometry::NurbsSurface(_))
}
pub(super) fn require_polygon_options(options: Options) -> Result<(), CommandError> {
    if options.crease_corners || options.loose || !options.split || !options.join_outputs {
        Err(CommandError::Usage(
            "CreaseCorners, Loose, Split and JoinOutputs require planar surface faces",
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
) -> Result<(Vec<Geometry>, Action, ComponentSelectionKind, usize), CommandError> {
    let source;
    let brep = match geometry {
        Geometry::Brep(b) => b,
        Geometry::NurbsSurface(s) => {
            source = Brep::try_surface_face(s.clone(), tolerance)?;
            &source
        }
        _ => return Err(CommandError::UnsupportedInsetGeometry),
    };
    use viboceros_geometry::{PlanarInsetOutput, PlanarInsetOutputMode};
    let mode = if !options.split {
        PlanarInsetOutputMode::Curves
    } else if !options.join_outputs {
        PlanarInsetOutputMode::Separate
    } else {
        PlanarInsetOutputMode::Joined
    };
    let result = brep.try_inset_planar_faces_output(
        faces,
        viboceros_geometry::PlanarInsetOptions {
            distance: options.distance,
            crease_corners: options.crease_corners,
            loose: options.loose,
        },
        mode,
        tolerance,
        viboceros_geometry::PlanarInsetOperations {
            curves: offset_curve::offset_operations(),
            edit_points: edit_point_solver(),
        },
    )?;
    let geometries = match result {
        PlanarInsetOutput::Breps(parts) => {
            parts.into_iter().map(Geometry::Brep).collect::<Vec<_>>()
        }
        PlanarInsetOutput::Curves(curves) => curves.into_iter().map(Geometry::NurbsCurve).collect(),
    };
    let bytes = geometries
        .iter()
        .fold(0usize, |sum, g| sum.saturating_add(storage(g)));
    let action = match mode {
        PlanarInsetOutputMode::Joined => Action::Replace,
        PlanarInsetOutputMode::Separate => Action::Recreate,
        PlanarInsetOutputMode::Curves => Action::Retain,
    };
    Ok((geometries, action, ComponentSelectionKind::BrepFace, bytes))
}
fn storage(geometry: &Geometry) -> usize {
    if let Geometry::NurbsCurve(c) = geometry {
        return 128usize
            .saturating_add(c.control_points().len().saturating_mul(32))
            .saturating_add(c.knots().len().saturating_mul(8));
    }
    let Geometry::Brep(result) = geometry else {
        unreachable!()
    };
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
    bytes
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
