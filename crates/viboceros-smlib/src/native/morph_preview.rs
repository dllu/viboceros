//! Reuse checked native basis edits and extraction for prepared display cages.
use super::*;
use viboceros_geometry::{GeometryError, MorphPreviewOperations};

fn geometry_error(error: Error) -> GeometryError {
    match error {
        Error::Geometry(error) => error,
        error => GeometryError::InterpolationBackendFailed {
            message: error.to_string(),
        },
    }
}

// Matches the pinned kernel build's NL_MAXDEG. Unsupported preview bases
// keep their established portable producers; native failures still propagate.
const NATIVE_DEGREE_LIMIT: usize = 17;

pub fn morph_preview_operations() -> MorphPreviewOperations {
    MorphPreviewOperations {
        interpolate: morph_interpolation,
        elevate_curve: |curve, degree| {
            if curve.degree() > NATIVE_DEGREE_LIMIT || degree > NATIVE_DEGREE_LIMIT {
                curve.try_change_degree(degree, false)
            } else {
                change_curve_degree(curve, degree, false).map_err(geometry_error)
            }
        },
        elevate_surface: |surface, u, v| {
            if [surface.degree_u(), surface.degree_v(), u, v]
                .into_iter()
                .any(|d| d > NATIVE_DEGREE_LIMIT)
            {
                surface.try_change_degree(u, v, false)
            } else {
                change_surface_degree(surface, u, v, false).map_err(geometry_error)
            }
        },
        isocurve: |surface, direction, parameter| {
            if surface.degree_u() > NATIVE_DEGREE_LIMIT || surface.degree_v() > NATIVE_DEGREE_LIMIT
            {
                if direction == viboceros_geometry::SurfaceKnotDirection::U {
                    surface.isocurve_u(parameter)
                } else {
                    surface.isocurve_v(parameter)
                }
            } else {
                surface_isocurve(surface, direction, parameter).map_err(geometry_error)
            }
        },
        trim_curve: |curve, interval| {
            if curve.degree() > NATIVE_DEGREE_LIMIT {
                curve.try_trimmed(interval)
            } else {
                trim_curve(curve, interval).map_err(geometry_error)
            }
        },
    }
}

/// Preserve the existing standalone curve-cage degree policy.
pub fn prepare_morph_preview_curve(source: &NurbsCurve) -> Result<NurbsCurve, GeometryError> {
    (morph_preview_operations().elevate_curve)(source, source.degree().max(3))
}
