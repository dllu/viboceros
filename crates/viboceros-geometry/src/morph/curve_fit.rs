use super::interpolation::{
    Axis, Break, control_count, error_fractions, knots, seed_breaks, stable_lerp,
};
use super::*;
use crate::ParameterSide;
use faer::Mat;
use std::collections::HashMap;

#[cfg(test)]
mod tests;

const ERROR_SAMPLES: usize = 16;
mod rational;
mod validate;

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
    // Rational sources need not lose their exact representation just because
    // their point map is nonlinear. These optional bounded candidates still
    // pass the same native-parameter, sided Euclidean validation as refinement.
    if source.is_rational() && source.degree() <= 3 {
        if source.control_points().len() <= maximum
            && let Ok(candidate) = rational::mapped_controls(morph, source)
            && validate::errors(
                &mut point_at,
                &candidate,
                &breaks,
                &fractions,
                tolerance.absolute(),
            )?
            .deviation
                <= tolerance.absolute()
        {
            return Ok(candidate);
        }
        if let Some(candidate) = rational::candidate(&mut point_at, source, maximum)?
            && validate::errors(
                &mut point_at,
                &candidate,
                &breaks,
                &fractions,
                tolerance.absolute() * 0.8,
            )?
            .deviation
                <= tolerance.absolute() * 0.8
        {
            return Ok(candidate);
        }
    }
    loop {
        let count = control_count(&breaks);
        if count > maximum {
            return Err(GeometryError::TooManyMorphCurveControlPoints { maximum });
        }
        let knots = knots(&breaks);
        let approximation = interpolate(&mut point_at, knots)?;
        let validate::Errors {
            deviation,
            mut refinements,
        } = validate::errors(
            &mut point_at,
            &approximation,
            &breaks,
            &fractions,
            tolerance.absolute() * 0.8,
        )?;
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
    let axis = Axis::cubic(knots)?;
    let count = axis.stations.len();
    let mut targets = Vec::with_capacity(count);
    for station in &axis.stations {
        targets.push(point_at(station.parameter, station.side)?);
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
    let solution = axis.solve(rhs)?;
    let controls = (0..count)
        .map(|i| {
            if axis.stations[i].fixed {
                Ok(targets[i])
            } else {
                Point3::try_from(std::array::from_fn(|axis| {
                    solution[(i, axis)] + origin[axis]
                }))
            }
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    NurbsCurve::try_new(axis.degree, controls, axis.knots)
}
