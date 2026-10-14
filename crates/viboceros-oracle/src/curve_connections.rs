//! Match oracle producers with the command backends.
use viboceros_command::CommandError;
use viboceros_geometry::{
    Curve3, CurveArcExtensionStyle, CurveFilletExtensionStyles, CurveOtherExtensionStyle, Point3,
    PolyCurve3, Real, Tolerance,
};
pub(super) fn fillet_corners(
    source: &PolyCurve3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<PolyCurve3, CommandError> {
    #[cfg(feature = "native-smlib")]
    return viboceros_smlib::fillet_corners(source, radius, tolerance).map_err(native_error);
    #[cfg(not(feature = "native-smlib"))]
    Ok(source.try_fillet_corners(radius, tolerance)?)
}
pub(super) fn connect_parts(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    arc_extension: CurveArcExtensionStyle,
    other_extension: CurveOtherExtensionStyle,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, CommandError> {
    #[cfg(feature = "native-smlib")]
    return viboceros_smlib::connect_curves_parts(
        first,
        second,
        arc_extension,
        other_extension,
        tolerance,
    )
    .map_err(native_error);
    #[cfg(not(feature = "native-smlib"))]
    Ok(viboceros_geometry::try_connect_curves_parts_with_styles(
        first.0,
        first.1,
        second.0,
        second.1,
        arc_extension,
        other_extension,
        tolerance,
    )?)
}

pub(super) fn fillet_joined(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    radius: Real,
    styles: CurveFilletExtensionStyles,
    tolerance: Tolerance,
) -> Result<PolyCurve3, CommandError> {
    #[cfg(feature = "native-smlib")]
    return viboceros_smlib::fillet_curves_joined(first, second, radius, styles, tolerance)
        .map_err(native_error);
    #[cfg(not(feature = "native-smlib"))]
    Ok(viboceros_geometry::try_fillet_curves_joined_with_styles(
        first.0, first.1, second.0, second.1, radius, styles, tolerance,
    )?)
}

pub(super) fn fillet_parts(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    radius: Real,
    trim: bool,
    styles: CurveFilletExtensionStyles,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, CommandError> {
    #[cfg(feature = "native-smlib")]
    return viboceros_smlib::fillet_curves_parts(first, second, radius, trim, styles, tolerance)
        .map_err(native_error);
    #[cfg(not(feature = "native-smlib"))]
    Ok(viboceros_geometry::try_fillet_curves_parts_with_styles(
        first, second, radius, trim, styles, tolerance,
    )?)
}

#[cfg(feature = "native-smlib")]
fn native_error(error: viboceros_smlib::Error) -> CommandError {
    match error {
        viboceros_smlib::Error::Geometry(error) => CommandError::Geometry(error),
        other => CommandError::from(other),
    }
}
