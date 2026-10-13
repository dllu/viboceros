//! Shared interpolation backend for point input and curve-through commands.
use super::*;

pub fn command_curve(
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

pub(super) fn fit_curve(
    source: CurveRef<'_>,
    degree: usize,
    fit_tolerance: Real,
    angle_tolerance: Real,
    tolerance: Tolerance,
) -> Result<NurbsCurve, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::fit_curve(source, degree, fit_tolerance, angle_tolerance, tolerance)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(viboceros_geometry::try_fit_curve(
            source,
            degree,
            fit_tolerance,
            angle_tolerance,
            tolerance,
        )?)
    }
}

pub(super) fn tween_curves(
    start: &NurbsCurve,
    end: &NurbsCurve,
    number: usize,
    method: CurveTweenMatchMethod,
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::curve_tweens(start, end, number, method, tolerance)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(viboceros_geometry::try_tween_nurbs_curves(
            start, end, number, method, tolerance,
        )?)
    }
}
