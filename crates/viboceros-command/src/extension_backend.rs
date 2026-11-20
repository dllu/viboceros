//! Optional native extrapolation around the shared Extend/ExtendSrf policies.
use super::*;
use std::ops::RangeInclusive;

pub(super) fn curve_domain(
    source: &NurbsCurve,
    interval: RangeInclusive<Real>,
) -> Result<NurbsCurve, CommandError> {
    return viboceros_smlib::extend_curve(source, interval).map_err(native_error);
}

pub(super) fn curve_length(
    source: &NurbsCurve,
    side: CurveExtensionSide,
    length: Real,
    tolerance: Tolerance,
    natural: bool,
    joined: bool,
) -> Result<NurbsCurve, CommandError> {
    let construct = match (natural, joined) {
        (true, false) => viboceros_smlib::extend_curve_naturally_by_length,
        (false, false) => viboceros_smlib::extend_curve_by_length,
        (true, true) => viboceros_smlib::join_curve_natural_extension,
        (false, true) => viboceros_smlib::join_curve_smooth_extension,
    };
    return construct(source, side, length, tolerance).map_err(native_error);
}

pub(super) fn curve_pieces(
    source: &NurbsCurve,
    side: CurveExtensionSide,
    length: Real,
    tolerance: Tolerance,
    natural: bool,
) -> Result<Vec<NurbsCurve>, CommandError> {
    let construct = if natural {
        viboceros_smlib::separate_curve_natural_extensions
    } else {
        viboceros_smlib::separate_curve_smooth_extensions
    };
    return construct(source, side, length, tolerance).map_err(native_error);
}

pub(super) fn surface_domain(
    source: &NurbsSurface,
    axis: SurfaceExtensionDirection,
    interval: RangeInclusive<Real>,
) -> Result<NurbsSurface, CommandError> {
    return viboceros_smlib::extend_surface(
        source,
        match axis {
            SurfaceExtensionDirection::U => SurfaceKnotDirection::U,
            SurfaceExtensionDirection::V => SurfaceKnotDirection::V,
        },
        interval,
    )
    .map_err(native_error);
}
pub(super) fn surface_length(
    source: &NurbsSurface,
    edge: SurfaceExtensionEdge,
    length: Real,
) -> Result<NurbsSurface, CommandError> {
    return viboceros_smlib::extend_surface_by_length(source, edge, length).map_err(native_error);
}

fn native_error(error: viboceros_smlib::Error) -> CommandError {
    match error {
        viboceros_smlib::Error::Geometry(error) => CommandError::Geometry(error),
        other => CommandError::from(other),
    }
}

pub(super) fn curve_boundary(
    source: &NurbsCurve,
    side: CurveExtensionSide,
    style: curve_extend::Style,
    boundaries: &[GeometryCurveExtensionBoundary],
    tolerance: Tolerance,
    joined: bool,
) -> Result<NurbsCurve, CommandError> {
    let construct = if joined {
        viboceros_smlib::join_curve_to_boundaries
    } else {
        viboceros_smlib::extend_curve_to_boundaries
    };
    return construct(source, side, style, boundaries, tolerance).map_err(native_error);
}

pub(super) fn curve_boundary_pieces(
    source: &NurbsCurve,
    side: CurveExtensionSide,
    style: curve_extend::Style,
    boundaries: &[GeometryCurveExtensionBoundary],
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, CommandError> {
    return viboceros_smlib::separate_curve_extensions_to_boundaries(
        source, side, style, boundaries, tolerance,
    )
    .map_err(native_error);
}
