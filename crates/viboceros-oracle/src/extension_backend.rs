//! Use the command's extension producers in both geometry probes and timings.
use std::ops::RangeInclusive;
use viboceros_command::CommandError;
use viboceros_geometry::{
    CurveExtensionSide, NurbsCurve, NurbsSurface, SurfaceExtensionEdge, Tolerance,
};

pub(super) fn curve_domain(
    source: &NurbsCurve,
    interval: RangeInclusive<f64>,
) -> Result<NurbsCurve, CommandError> {
    return viboceros_smlib::extend_curve(source, interval).map_err(native_error);
}

pub(super) fn curve_length(
    source: &NurbsCurve,
    side: CurveExtensionSide,
    length: f64,
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
    length: f64,
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
    axis: crate::SurfaceKnotAxis,
    interval: RangeInclusive<f64>,
) -> Result<NurbsSurface, CommandError> {
    return viboceros_smlib::extend_surface(
        source,
        match axis {
            crate::SurfaceKnotAxis::U => viboceros_geometry::SurfaceKnotDirection::U,
            crate::SurfaceKnotAxis::V => viboceros_geometry::SurfaceKnotDirection::V,
        },
        interval,
    )
    .map_err(native_error);
}

pub(super) fn surface_length(
    source: &NurbsSurface,
    edge: SurfaceExtensionEdge,
    length: f64,
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
    style: viboceros_geometry::CurveExtensionStyle,
    boundaries: &[viboceros_geometry::CurveExtensionBoundary],
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
    style: viboceros_geometry::CurveExtensionStyle,
    boundaries: &[viboceros_geometry::CurveExtensionBoundary],
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, CommandError> {
    return viboceros_smlib::separate_curve_extensions_to_boundaries(
        source, side, style, boundaries, tolerance,
    )
    .map_err(native_error);
}
