//! Optional original-net and cubic-composition candidates for rational curves.

use super::super::denominator;
use super::*;

pub(super) fn mapped_controls(
    morph: &(impl PointMorph + ?Sized),
    source: &NurbsCurve,
) -> Result<NurbsCurve, GeometryError> {
    let controls = source
        .control_points()
        .iter()
        .map(|c| WeightedPoint3::try_new(morph.morph_point(c.point())?, c.weight()))
        .collect::<Result<Vec<_>, _>>()?;
    NurbsCurve::try_new_rational(source.degree(), controls, source.knots().to_vec())
}

pub(super) fn candidate(
    point_at: &mut impl FnMut(Real, ParameterSide) -> Result<Point3, GeometryError>,
    source: &NurbsCurve,
    maximum: usize,
) -> Result<Option<NurbsCurve>, GeometryError> {
    let Some(denominator) = denominator::curve_weights(source) else {
        return Ok(None);
    };
    let Some(axis) =
        Axis::cubic_composition(source.degree(), source.knots(), source.domain(), maximum)
    else {
        return Ok(None);
    };
    let mut weights = Vec::with_capacity(axis.stations.len());
    for station in &axis.stations {
        let weight = denominator
            .evaluate_on_side(station.parameter, station.side)?
            .x()
            .powi(3);
        if weight == 0.0 || !weight.is_finite() {
            return Ok(None);
        }
        weights.push(weight);
    }
    let targets = axis
        .stations
        .iter()
        .map(|s| point_at(s.parameter, s.side))
        .collect::<Result<Vec<_>, _>>()?;
    // A failed source map is not a failed interpolation candidate. Only the
    // numerical solve and reconstruction below may fall back to polynomial fitting.
    Ok(interpolate(axis, &targets, &weights).ok())
}

fn interpolate(
    axis: Axis,
    targets: &[Point3],
    weights: &[Real],
) -> Result<NurbsCurve, GeometryError> {
    let candidate = targets[0].to_array();
    let origin = if targets.iter().all(|p| {
        p.to_array()
            .into_iter()
            .zip(candidate)
            .all(|(a, b)| (a - b).is_finite())
    }) {
        candidate
    } else {
        [0.0; 3]
    };
    let rhs = Mat::from_fn(targets.len(), 4, |row, column| {
        if column == 3 {
            weights[row]
        } else {
            (targets[row].to_array()[column] - origin[column]) * weights[row]
        }
    });
    let solution = axis.solve(rhs)?;
    let controls = axis
        .stations
        .iter()
        .enumerate()
        .map(|(i, station)| {
            let weight = solution[(i, 3)];
            if weight <= 0.0 || !weight.is_finite() {
                return Err(GeometryError::ZeroWeightAtParameter);
            }
            let point = if station.fixed {
                targets[i]
            } else {
                Point3::try_from(std::array::from_fn(|j| {
                    solution[(i, j)] / weight + origin[j]
                }))?
            };
            WeightedPoint3::try_new(point, weight)
        })
        .collect::<Result<Vec<_>, _>>()?;
    NurbsCurve::try_new_rational(axis.degree, controls, axis.knots)
}
