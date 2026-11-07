//! Closed-form parallel tangent radius differences.
use super::radius::ArcBlendRadiusSolution;
use crate::{GeometryError, Point3, Real, Tolerance, Vector3};

/// Solve R0-R1 without iteration. Parallel tangent biarcs have radii a*R
/// and (1-a)*R, where R=|chord|²/(2*|tangent cross chord|).
/// The radius solver's ratio interval is [0.01,100], observed independently
/// in Rhino command captures. Ordinary explicit ratios are not bounded here.
/// Other tangent configurations require a separate numerical solver.
pub fn try_parallel_arc_blend_radius_ratio(
    start: Point3,
    start_direction: Vector3,
    end: Point3,
    end_direction: Vector3,
    difference: Real,
    tolerance: Tolerance,
) -> Result<ArcBlendRadiusSolution, GeometryError> {
    crate::require_finite([difference], "arc blend radius difference")?;
    let first = start_direction.normalized_nonzero()?.as_vector();
    let second = end_direction.normalized_nonzero()?.as_vector();
    if first.dot(second)? <= 0. || first.cross(second)?.length()? > 1e-12 {
        return Err(GeometryError::Degenerate {
            context: "arc blend radius differences require parallel forward tangents",
        });
    }
    let chord = start.vector_to(end)?;
    let length = chord.length()?;
    let direction = chord.normalized_nonzero()?.as_vector();
    let sine = first.cross(direction)?.length()?;
    if sine <= 1e-12 || first.dot(direction)? <= 0. {
        return Err(GeometryError::Degenerate {
            context: "arc blend radius difference chord",
        });
    }
    let radius = length / (2. * sine);
    crate::require_finite([radius], "arc blend radius sum")?;
    let a = (0.5 * (1. + difference / radius)).clamp(1. / 101., 100. / 101.);
    let achieved = (2. * a - 1.) * radius;
    Ok(ArcBlendRadiusSolution {
        ratio: a / (1. - a),
        radius_difference: achieved,
        reached_target: (achieved - difference).abs() <= tolerance.absolute(),
    })
}
