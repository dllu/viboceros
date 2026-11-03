//! Adaptive original-basis normal loci for curvature-partitioned Inset caps.
use super::*;
use crate::spline_collocation::Axis;
mod solve;
use faer::Mat;

const MAX_CONTROLS: usize = 1000;

pub(crate) fn fit_normal_locus(
    source: &NurbsCurve,
    distance: Real,
    normal: UnitVector3,
    tolerance: Tolerance,
    solver: Option<crate::CurveEditPointSolver>,
) -> Result<NurbsCurve, GeometryError> {
    if source.degree() > 17 || source.control_points().len() > MAX_CONTROLS {
        return Err(GeometryError::NurbsOffsetFitLimit);
    }
    let source = source.clamped_to_active_domain()?;
    let mut knots = source.knots().to_vec();
    for _ in 0..32 {
        let candidate = interpolate(&source, knots.clone(), distance, normal, solver)?;
        let mut inserted = Vec::new();
        for (a, b) in candidate.spans() {
            let mut largest = 0_f64;
            for i in 0..=4 {
                let t = match i {
                    0 => a,
                    4 => b,
                    _ => a.midpoint(b) + (i as Real - 2.) * (b - a) * 0.25,
                };
                let side = if i == 4 {
                    ParameterSide::Left
                } else {
                    ParameterSide::Right
                };
                let expected =
                    offset_sample_with_reversals(&source, distance, normal, t, side, true)?;
                let tangent = source.tangent_at_on_side(t, side)?;
                let left = normal.as_vector().cross(tangent.as_vector())?;
                let delta = expected.point.vector_to(candidate.evaluate(t)?)?;
                largest = largest.max(delta.dot(left)?.abs());
            }
            if largest > tolerance.absolute() {
                let t = a.midpoint(b);
                if t <= a || t >= b {
                    return Err(GeometryError::NurbsOffsetFitLimit);
                }
                inserted.push(t);
            }
        }
        if inserted.is_empty() {
            return Ok(candidate);
        }
        if candidate
            .control_points()
            .len()
            .saturating_add(inserted.len())
            > MAX_CONTROLS
        {
            return Err(GeometryError::NurbsOffsetFitLimit);
        }
        // Merge sorted knot streams once. Homogeneous collocation below
        // refines the denominator with no sequence of whole-curve insertions.
        let previous = std::mem::take(&mut knots);
        knots.reserve(previous.len() + inserted.len());
        let mut cursor = 0;
        for k in previous {
            while cursor < inserted.len() && inserted[cursor] < k {
                knots.push(inserted[cursor]);
                cursor += 1;
            }
            knots.push(k);
        }
        knots.extend_from_slice(&inserted[cursor..]);
    }
    Err(GeometryError::NurbsOffsetFitLimit)
}

fn interpolate(
    source: &NurbsCurve,
    knots: Vec<Real>,
    distance: Real,
    normal: UnitVector3,
    solver: Option<crate::CurveEditPointSolver>,
) -> Result<NurbsCurve, GeometryError> {
    let p = source.degree();
    let axis = Axis::sites(p, knots)?;
    let count = axis.stations.len();
    let origin = source.evaluate(*source.domain().start())?;
    let scale = source
        .control_points()
        .iter()
        .map(|c| c.weight().abs())
        .fold(0., Real::max);
    let mut rhs = Mat::zeros(count, 4);
    for (i, station) in axis.stations.iter().enumerate() {
        let weight = source.evaluate_scaled_homogeneous(station.parameter, scale)?[3];
        let sample = offset_sample_with_reversals(
            source,
            distance,
            normal,
            station.parameter,
            station.side,
            true,
        )?;
        let point = origin.vector_to(sample.point)?.to_array();
        for k in 0..3 {
            rhs[(i, k)] = point[k] * weight;
        }
        rhs[(i, 3)] = weight;
    }
    let result = solve::solve(&axis, rhs, solver)?;
    let controls = (0..count)
        .map(|i| {
            let w = result[(i, 3)];
            let point = origin.translated(Vector3::try_from(std::array::from_fn(|k| {
                result[(i, k)] / w
            }))?)?;
            WeightedPoint3::try_new(point, w * scale)
        })
        .collect::<Result<Vec<_>, _>>()?;
    NurbsCurve::try_new_rational(p, controls, axis.knots)
}

#[cfg(test)]
mod tests;
