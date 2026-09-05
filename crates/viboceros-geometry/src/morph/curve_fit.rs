use super::interpolation::{
    Break, DEGREE, collocation_row, control_count, error_fractions, knots, seed_breaks, solve,
    stable_lerp,
};
use super::*;
use crate::ParameterSide;
use faer::Mat;
use std::collections::HashMap;

#[cfg(test)]
mod tests;

const ERROR_SAMPLES: usize = 16;

pub(super) fn fit(
    morph: &(impl PointMorph + ?Sized),
    source: &NurbsCurve,
    tolerance: Tolerance,
    maximum: usize,
) -> Result<NurbsCurve, GeometryError> {
    // PointMorph is deterministic. Dyadic refinement reuses many stations;
    // cache the source evaluation and point map together, retaining the side
    // bit so independent limits at full-order knots never alias.
    let mut cache = HashMap::new();
    let mut point_at = |t: Real, side: ParameterSide| {
        let key = (t.to_bits(), side == ParameterSide::Left);
        if let Some(point) = cache.get(&key) {
            return Ok(*point);
        }
        let point = morph.morph_point(source.evaluate_on_side(t, side)?)?;
        cache.insert(key, point);
        Ok::<_, GeometryError>(point)
    };
    let mut breaks = seed_breaks(source.degree(), source.knots(), source.domain());
    let fractions = error_fractions(ERROR_SAMPLES);
    loop {
        let count = control_count(&breaks);
        if count > maximum {
            return Err(GeometryError::TooManyMorphCurveControlPoints { maximum });
        }
        let knots = knots(&breaks);
        let approximation = interpolate(&mut point_at, knots)?;
        let mut refinements = Vec::new();
        let mut deviation: Real = 0.0;
        for (index, interval) in breaks.windows(2).enumerate() {
            let (start, end) = (interval[0].parameter, interval[1].parameter);
            let mut error: Real = 0.0;
            for &fraction in &fractions {
                let t = stable_lerp(start, end, fraction)?;
                let side = if t == end {
                    ParameterSide::Left
                } else {
                    ParameterSide::Right
                };
                let exact = point_at(t, side)?;
                let actual = approximation.evaluate_on_side(t, side)?;
                error = error.max(exact.distance_to(actual)?);
            }
            deviation = deviation.max(error);
            // Modest sampling headroom; this is not a continuous error proof.
            if error > tolerance.absolute() * 0.8 {
                refinements.push((index, error));
            }
        }
        if refinements.is_empty() {
            return Ok(approximation);
        }
        let failure = || GeometryError::CurveMorphDidNotConverge {
            tolerance: tolerance.absolute(),
            deviation,
            maximum,
        };
        if count == maximum {
            if deviation <= tolerance.absolute() {
                return Ok(approximation);
            }
            return Err(failure());
        }
        refinements.sort_by(|a, b| b.1.total_cmp(&a.1));
        refinements.truncate(maximum - count);
        let mut added = Vec::with_capacity(refinements.len());
        for (index, _) in refinements {
            let (a, b) = (breaks[index].parameter, breaks[index + 1].parameter);
            let t = stable_lerp(a, b, 0.5)?;
            if t > a && t < b {
                added.push(Break {
                    parameter: t,
                    multiplicity: 1,
                });
            }
        }
        if added.is_empty() {
            if deviation <= tolerance.absolute() {
                return Ok(approximation);
            }
            return Err(failure());
        }
        breaks.extend(added);
        breaks.sort_by(|a, b| a.parameter.total_cmp(&b.parameter));
    }
}

fn interpolate(
    point_at: &mut impl FnMut(Real, ParameterSide) -> Result<Point3, GeometryError>,
    knots: Vec<Real>,
) -> Result<NurbsCurve, GeometryError> {
    let count = knots.len() - DEGREE - 1;
    let mut rows = Vec::with_capacity(count);
    let mut targets = Vec::with_capacity(count);
    let mut fixed = Vec::with_capacity(count);
    for i in 0..count {
        let (basis, t, side, is_fixed) = collocation_row(&knots, i)?;
        rows.push(basis);
        targets.push(point_at(t, side)?);
        fixed.push(is_fixed);
    }
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
    let rhs = Mat::from_fn(count, 3, |row, column| {
        targets[row].to_array()[column] - origin[column]
    });
    let solution = solve(&rows, rhs)?;
    let controls = (0..count)
        .map(|i| {
            if fixed[i] {
                Ok(targets[i])
            } else {
                Point3::try_from(std::array::from_fn(|axis| {
                    solution[(i, axis)] + origin[axis]
                }))
            }
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    NurbsCurve::try_new(DEGREE, controls, knots)
}
