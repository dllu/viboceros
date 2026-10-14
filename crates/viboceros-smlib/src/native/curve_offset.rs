//! Native pointwise NURBS offset fitting and edits with shared admission/corner policy.
use super::*;
use viboceros_geometry::{
    Curve3, CurveOffsetCornerStyle, CurveOffsetOperations, GeometryError, Point3, UnitVector3,
};

fn geometry_error(error: Error) -> GeometryError {
    match error {
        Error::Geometry(error) => error,
        error => GeometryError::InterpolationBackendFailed {
            message: error.to_string(),
        },
    }
}
const NATIVE_DEGREE_LIMIT: usize = 17;
pub fn curve_offset_operations() -> CurveOffsetOperations {
    CurveOffsetOperations {
        intersect: |first, second, tolerance| {
            curve_intersection_events(first, second, tolerance).map_err(geometry_error)
        },
        bezier: |curve| {
            if curve.degree() > NATIVE_DEGREE_LIMIT {
                curve.try_bezier_spans()
            } else {
                curve_bezier_spans(curve).map_err(geometry_error)
            }
        },
        smooth: |curve, distance, normal, tolerance, closed| {
            super::curve_offset_fit::fit(curve, distance, normal, tolerance, closed)
                .map_err(geometry_error)
        },
        trim: |curve, interval| {
            if curve.degree() > NATIVE_DEGREE_LIMIT {
                curve.try_trimmed(interval)
            } else {
                trim_curve(curve, interval).map_err(geometry_error)
            }
        },
        split: |curve, parameter| {
            if curve.degree() > NATIVE_DEGREE_LIMIT {
                curve.try_split(parameter)
            } else {
                split_curve(curve, parameter).map_err(geometry_error)
            }
        },
        split_at: |curve, parameters| {
            if curve.degree() > NATIVE_DEGREE_LIMIT {
                curve.try_split_at_parameters(parameters)
            } else {
                split_curve_at_parameters(curve, parameters).map_err(geometry_error)
            }
        },
    }
}
pub fn offset_curve_parts(
    source: &Curve3,
    distance: f64,
    normal: UnitVector3,
    tolerance: Tolerance,
    corner: CurveOffsetCornerStyle,
) -> Result<Vec<Curve3>, GeometryError> {
    source.try_offset_parts_with_operations(
        distance,
        normal,
        tolerance,
        corner,
        Some(curve_offset_operations()),
    )
}
pub fn offset_curve_through_point(
    source: &Curve3,
    through: Point3,
    normal: UnitVector3,
    tolerance: Tolerance,
    corner: CurveOffsetCornerStyle,
) -> Result<(f64, Vec<Curve3>), GeometryError> {
    source.try_offset_through_point_with_operations(
        through,
        normal,
        tolerance,
        corner,
        Some(curve_offset_operations()),
    )
}
