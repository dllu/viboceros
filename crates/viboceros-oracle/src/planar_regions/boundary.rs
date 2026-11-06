//! Spatial membership of complete closed boundaries or constituent open pieces.
use super::*;
pub(super) fn matches(
    source: &NurbsCurve,
    edges: &[&NurbsCurve],
    tolerance: Tolerance,
    subset: bool,
) -> Result<bool, GeometryError> {
    if edges.len() == 1 && *edges[0] == *source {
        return Ok(true);
    }
    let epsilon = if subset { tolerance.absolute() } else { 1e-7 };
    let on = |point: Point3, candidates: &[&NurbsCurve]| -> Result<bool, GeometryError> {
        for c in candidates {
            let parameter = c.closest_parameter(point, tolerance)?;
            let distance = c.evaluate(parameter)?.distance_to(point)?;
            if distance < epsilon || (subset && distance == epsilon) {
                return Ok(true);
            }
        }
        Ok(false)
    };
    for i in 0..=32 {
        if !on(
            source.evaluate(source.parameter_at(i as f64 / 32.)?)?,
            edges,
        )? {
            return Ok(false);
        }
    }
    if subset {
        return Ok(true);
    }
    for edge in edges {
        for i in 0..=16 {
            if !on(
                edge.evaluate(edge.parameter_at(i as f64 / 16.)?)?,
                &[source],
            )? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
