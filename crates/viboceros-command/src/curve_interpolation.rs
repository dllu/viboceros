//! Shared interpolation backend for point input and curve-through commands.
use super::*;

pub(super) fn command_curve(
    points: &[Point3],
    options: viboceros_geometry::CurveInterpolationOptions,
) -> Result<NurbsCurve, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::interpolate_curve_for_command(points, options)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(NurbsCurve::try_interpolate_for_command(
            points,
            options,
            Tolerance::DEFAULT,
        )?)
    }
}

pub(super) fn through_points(
    points: &[Point3],
    degree: usize,
    construction: CurveThroughConstruction,
    closed: bool,
) -> Result<NurbsCurve, CommandError> {
    viboceros_geometry::try_curve_through_points_with_interpolation(
        points,
        degree,
        construction,
        closed,
        &mut command_curve,
    )
}
