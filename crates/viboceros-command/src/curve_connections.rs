//! Select the same connected-end producers for all three commands.
use super::*;
use viboceros_geometry::{
    CurveArcExtensionStyle, CurveFilletExtensionStyles, CurveOtherExtensionStyle, PolyCurve3,
};

pub(super) fn fillet_corners(
    source: &PolyCurve3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<PolyCurve3, CommandError> {
    return viboceros_smlib::fillet_corners(source, radius, tolerance).map_err(native_error);
}

pub(super) fn connect_parts(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    arc_extension: CurveArcExtensionStyle,
    other_extension: CurveOtherExtensionStyle,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, CommandError> {
    return viboceros_smlib::connect_curves_parts(
        first,
        second,
        arc_extension,
        other_extension,
        tolerance,
    )
    .map_err(native_error);
}

pub(super) fn connect_joined(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    arc_extension: CurveArcExtensionStyle,
    other_extension: CurveOtherExtensionStyle,
    tolerance: Tolerance,
) -> Result<PolyCurve3, CommandError> {
    return viboceros_smlib::connect_curves_joined(
        first,
        second,
        arc_extension,
        other_extension,
        tolerance,
    )
    .map_err(native_error);
}

pub(super) fn fillet_parts(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    radius: Real,
    trim: bool,
    styles: CurveFilletExtensionStyles,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, CommandError> {
    return viboceros_smlib::fillet_curves_parts(first, second, radius, trim, styles, tolerance)
        .map_err(native_error);
}

pub(super) fn fillet_joined(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    radius: Real,
    styles: CurveFilletExtensionStyles,
    tolerance: Tolerance,
) -> Result<PolyCurve3, CommandError> {
    return viboceros_smlib::fillet_curves_joined(first, second, radius, styles, tolerance)
        .map_err(native_error);
}

fn native_error(error: viboceros_smlib::Error) -> CommandError {
    match error {
        viboceros_smlib::Error::Geometry(error) => CommandError::Geometry(error),
        other => CommandError::from(other),
    }
}
