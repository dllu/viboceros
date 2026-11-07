//! Euclidean setbacks are endpoint sphere crossings, not arc-length stations.
use super::*;

pub(super) fn parameter(
    curve: CurveRef<'_>,
    distance: Real,
    from_end: bool,
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    let domain = curve.domain();
    if distance == 0. {
        return Ok(if from_end {
            *domain.end()
        } else {
            *domain.start()
        });
    }
    let station = curve
        .point_at_chord_distance(distance, from_end, tolerance)?
        .ok_or_else(unsupported)?;
    if station.parameter
        == if from_end {
            *domain.start()
        } else {
            *domain.end()
        }
    {
        return Err(unsupported());
    }
    Ok(station.parameter)
}
