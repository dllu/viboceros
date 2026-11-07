//! An infinite straight support can meet a circular arc behind either end.
use super::*;
pub(super) fn connected<E: From<GeometryError>>(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    styles: CurveChamferExtensionStyles,
    tolerance: Tolerance,
    operations: &mut CurveExtensionOperations<'_, E>,
) -> Result<([Curve3; 2], [Point3; 2]), E> {
    let initial = connected_ends_with_operations(
        first,
        second,
        styles.arc,
        styles.other,
        tolerance,
        operations,
    );
    // Analytic arc/line alternatives cannot mask a delegated producer error.
    if initial.is_ok()
        || !matches!(
            (first.0, second.0),
            (Curve3::Arc(_), Curve3::Line(_)) | (Curve3::Line(_), Curve3::Arc(_))
        )
    {
        return initial;
    }
    // Preserve curved-end picking. Only a straight source has an unambiguous
    // alternative infinite support with the same geometry.
    let opposite = |(curve, pick): (&Curve3, Point3)| -> Result<Option<Point3>, GeometryError> {
        if !matches!(curve, Curve3::Line(_)) {
            return Ok(None);
        }
        Ok(Some(if selected_end(curve, pick, tolerance)? {
            curve.as_ref().start_point()?
        } else {
            curve.as_ref().end_point()?
        }))
    };
    for (a, b) in [(opposite(first)?, None), (None, opposite(second)?)] {
        if a.is_none() && b.is_none() {
            continue;
        }
        if let Ok(value) = connected_ends_with_operations(
            (first.0, a.unwrap_or(first.1)),
            (second.0, b.unwrap_or(second.1)),
            styles.arc,
            styles.other,
            tolerance,
            operations,
        ) {
            return Ok(value);
        }
    }
    initial
}
