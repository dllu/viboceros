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
    #[cfg(feature = "native-smlib")]
    return viboceros_smlib::extend_curve(source, interval).map_err(native_error);
    #[cfg(not(feature = "native-smlib"))]
    Ok(source.try_extended_to(interval)?)
}

pub(super) fn curve_length(
    source: &NurbsCurve,
    side: CurveExtensionSide,
    length: f64,
    tolerance: Tolerance,
    natural: bool,
    joined: bool,
) -> Result<NurbsCurve, CommandError> {
    #[cfg(feature = "native-smlib")]
    let construct = match (natural, joined) {
        (true, false) => viboceros_smlib::extend_curve_naturally_by_length,
        (false, false) => viboceros_smlib::extend_curve_by_length,
        (true, true) => viboceros_smlib::join_curve_natural_extension,
        (false, true) => viboceros_smlib::join_curve_smooth_extension,
    };
    #[cfg(not(feature = "native-smlib"))]
    let construct = match (natural, joined) {
        (true, false) => NurbsCurve::try_merged_naturally_by_length,
        (false, false) => NurbsCurve::try_extended_by_length,
        (true, true) => NurbsCurve::try_joined_naturally_by_length,
        (false, true) => NurbsCurve::try_joined_smoothly_by_length,
    };
    #[cfg(feature = "native-smlib")]
    return construct(source, side, length, tolerance).map_err(native_error);
    #[cfg(not(feature = "native-smlib"))]
    Ok(construct(source, side, length, tolerance)?)
}

pub(super) fn curve_pieces(
    source: &NurbsCurve,
    side: CurveExtensionSide,
    length: f64,
    tolerance: Tolerance,
    natural: bool,
) -> Result<Vec<NurbsCurve>, CommandError> {
    #[cfg(feature = "native-smlib")]
    let construct = if natural {
        viboceros_smlib::separate_curve_natural_extensions
    } else {
        viboceros_smlib::separate_curve_smooth_extensions
    };
    #[cfg(not(feature = "native-smlib"))]
    let construct = if natural {
        NurbsCurve::try_separate_natural_extensions_by_length
    } else {
        NurbsCurve::try_separate_smooth_extensions_by_length
    };
    #[cfg(feature = "native-smlib")]
    return construct(source, side, length, tolerance).map_err(native_error);
    #[cfg(not(feature = "native-smlib"))]
    Ok(construct(source, side, length, tolerance)?)
}

pub(super) fn surface_domain(
    source: &NurbsSurface,
    axis: crate::SurfaceKnotAxis,
    interval: RangeInclusive<f64>,
) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    return viboceros_smlib::extend_surface(
        source,
        match axis {
            crate::SurfaceKnotAxis::U => viboceros_geometry::SurfaceKnotDirection::U,
            crate::SurfaceKnotAxis::V => viboceros_geometry::SurfaceKnotDirection::V,
        },
        interval,
    )
    .map_err(native_error);
    #[cfg(not(feature = "native-smlib"))]
    Ok(match axis {
        crate::SurfaceKnotAxis::U => source.try_extended_u(interval)?,
        crate::SurfaceKnotAxis::V => source.try_extended_v(interval)?,
    })
}

pub(super) fn surface_length(
    source: &NurbsSurface,
    edge: SurfaceExtensionEdge,
    length: f64,
) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    return viboceros_smlib::extend_surface_by_length(source, edge, length).map_err(native_error);
    #[cfg(not(feature = "native-smlib"))]
    Ok(source.try_extended_by_length(edge, length)?)
}

#[cfg(feature = "native-smlib")]
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
    #[cfg(feature = "native-smlib")]
    let construct = if joined {
        viboceros_smlib::join_curve_to_boundaries
    } else {
        viboceros_smlib::extend_curve_to_boundaries
    };
    #[cfg(not(feature = "native-smlib"))]
    let construct = if joined {
        NurbsCurve::try_joined_to_boundaries
    } else {
        NurbsCurve::try_merged_to_boundaries
    };
    #[cfg(feature = "native-smlib")]
    return construct(source, side, style, boundaries, tolerance).map_err(native_error);
    #[cfg(not(feature = "native-smlib"))]
    Ok(construct(source, side, style, boundaries, tolerance)?)
}

pub(super) fn curve_boundary_pieces(
    source: &NurbsCurve,
    side: CurveExtensionSide,
    style: viboceros_geometry::CurveExtensionStyle,
    boundaries: &[viboceros_geometry::CurveExtensionBoundary],
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, CommandError> {
    #[cfg(feature = "native-smlib")]
    return viboceros_smlib::separate_curve_extensions_to_boundaries(
        source, side, style, boundaries, tolerance,
    )
    .map_err(native_error);
    #[cfg(not(feature = "native-smlib"))]
    Ok(source.try_separate_extensions_to_boundaries(side, style, boundaries, tolerance)?)
}
