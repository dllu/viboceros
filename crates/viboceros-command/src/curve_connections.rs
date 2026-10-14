//! Select the same connected-end producers for all three commands.
use super::*;
use viboceros_geometry::{
    CurveArcExtensionStyle, CurveChamferExtensionStyles, CurveFilletExtensionStyles,
    CurveOtherExtensionStyle, PolyCurve3,
};

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

pub(super) fn connect_joined(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    arc_extension: CurveArcExtensionStyle,
    other_extension: CurveOtherExtensionStyle,
    tolerance: Tolerance,
) -> Result<PolyCurve3, CommandError> {
    #[cfg(feature = "native-smlib")]
    return viboceros_smlib::connect_curves_joined(
        first,
        second,
        arc_extension,
        other_extension,
        tolerance,
    )
    .map_err(native_error);
    #[cfg(not(feature = "native-smlib"))]
    Ok(viboceros_geometry::try_connect_curves_joined_with_styles(
        first.0,
        first.1,
        second.0,
        second.1,
        arc_extension,
        other_extension,
        tolerance,
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

pub(super) fn chamfer_parts(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    distances: [Real; 2],
    trim: bool,
    styles: CurveChamferExtensionStyles,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, CommandError> {
    #[cfg(feature = "native-smlib")]
    return viboceros_smlib::chamfer_curves_parts(
        first, second, distances, trim, styles, tolerance,
    )
    .map_err(native_error);
    #[cfg(not(feature = "native-smlib"))]
    Ok(viboceros_geometry::try_chamfer_curves_parts_with_styles(
        first, second, distances, trim, styles, tolerance,
    )?)
}

pub(super) fn chamfer_joined(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    distances: [Real; 2],
    styles: CurveChamferExtensionStyles,
    tolerance: Tolerance,
) -> Result<PolyCurve3, CommandError> {
    #[cfg(feature = "native-smlib")]
    return viboceros_smlib::chamfer_curves_joined(first, second, distances, styles, tolerance)
        .map_err(native_error);
    #[cfg(not(feature = "native-smlib"))]
    Ok(viboceros_geometry::try_chamfer_curves_joined_with_styles(
        first.0, first.1, second.0, second.1, distances, styles, tolerance,
    )?)
}

#[cfg(feature = "native-smlib")]
fn native_error(error: viboceros_smlib::Error) -> CommandError {
    match error {
        viboceros_smlib::Error::Geometry(error) => CommandError::Geometry(error),
        other => CommandError::from(other),
    }
}
