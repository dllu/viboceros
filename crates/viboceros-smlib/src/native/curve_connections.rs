//! Native exact NURBS edits within shared Connect/Fillet/Chamfer policies.
use super::boundary_extension::with_native_operations;
use super::*;
use viboceros_geometry::{
    Curve3, CurveArcExtensionStyle, CurveChamferExtensionStyles, CurveFilletExtensionStyles,
    CurveOtherExtensionStyle, PolyCurve3,
};

pub fn connect_curves_parts(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    arc_extension: CurveArcExtensionStyle,
    other_extension: CurveOtherExtensionStyle,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, Error> {
    with_native_operations(|operations| {
        viboceros_geometry::try_connect_curves_parts_with_operations(
            first,
            second,
            arc_extension,
            other_extension,
            tolerance,
            operations,
        )
    })
}

pub fn connect_curves_joined(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    arc_extension: CurveArcExtensionStyle,
    other_extension: CurveOtherExtensionStyle,
    tolerance: Tolerance,
) -> Result<PolyCurve3, Error> {
    with_native_operations(|operations| {
        viboceros_geometry::try_connect_curves_joined_with_operations(
            first,
            second,
            arc_extension,
            other_extension,
            tolerance,
            operations,
        )
    })
}

pub fn fillet_curves_parts(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    radius: f64,
    trim: bool,
    styles: CurveFilletExtensionStyles,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, Error> {
    with_native_operations(|operations| {
        viboceros_geometry::try_fillet_curves_parts_with_operations(
            first, second, radius, trim, styles, tolerance, operations,
        )
    })
}

pub fn fillet_curves_joined(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    radius: f64,
    styles: CurveFilletExtensionStyles,
    tolerance: Tolerance,
) -> Result<PolyCurve3, Error> {
    with_native_operations(|operations| {
        viboceros_geometry::try_fillet_curves_joined_with_operations(
            first, second, radius, styles, tolerance, operations,
        )
    })
}

pub fn chamfer_curves_parts(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    distances: [f64; 2],
    trim: bool,
    styles: CurveChamferExtensionStyles,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, Error> {
    with_native_operations(|operations| {
        viboceros_geometry::try_chamfer_curves_parts_with_operations(
            first, second, distances, trim, styles, tolerance, operations,
        )
    })
}

pub fn chamfer_curves_joined(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    distances: [f64; 2],
    styles: CurveChamferExtensionStyles,
    tolerance: Tolerance,
) -> Result<PolyCurve3, Error> {
    with_native_operations(|operations| {
        viboceros_geometry::try_chamfer_curves_joined_with_operations(
            first, second, distances, styles, tolerance, operations,
        )
    })
}
