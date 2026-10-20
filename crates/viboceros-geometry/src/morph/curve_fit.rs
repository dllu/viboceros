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
    fit_with_stations(morph, source, tolerance, maximum, &[])
}

pub(super) fn fit_with_stations(
    morph: &(impl PointMorph + ?Sized),
    source: &NurbsCurve,
    tolerance: Tolerance,
    maximum: usize,
    stations: &[Real],
) -> Result<NurbsCurve, GeometryError> {
    if stations
        .iter()
        .any(|t| !t.is_finite() || !source.domain().contains(t))
    {
        return Err(GeometryError::InvalidControlNet {
            context: "extra morph validation stations must lie in the source domain",
        });
    }
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
    let breaks = seed_breaks(source.degree(), source.knots(), source.domain());
    let fractions = error_fractions(ERROR_SAMPLES);
    // Rational sources need not lose their exact representation just because
    // their point map is nonlinear. These optional bounded candidates still
    // pass the same native-parameter, sided Euclidean validation as refinement.
    if stations.is_empty() && source.is_rational() && source.degree() <= 3 {
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
        if let Some(candidate) =
            rational::candidate(&mut point_at, source, maximum, morph.interpolation_solver())?
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
    fit_samples(
        &mut point_at,
        breaks,
        tolerance,
        maximum,
        stations,
        morph.interpolation_solver(),
        None,
    )
}

/// Share adaptive collocation with parameter-dependent constructions, such as
/// surface offsets. A point map alone cannot distinguish two parameters at a
/// self-intersection. The evaluator must be deterministic on each knot side.
pub(crate) fn fit_parameterized(
    source: &NurbsCurve,
    breaks: Vec<Break>,
    tolerance: Tolerance,
    maximum: usize,
    solver: Option<MorphInterpolationSolver>,
    evaluate: impl FnMut(Real, ParameterSide) -> Result<Point3, GeometryError>,
) -> Result<NurbsCurve, GeometryError> {
    fit_parameterized_with_derivatives(source, breaks, tolerance, maximum, solver, None, evaluate)
}

pub(crate) fn fit_parameterized_with_derivatives(
    source: &NurbsCurve,
    breaks: Vec<Break>,
    tolerance: Tolerance,
    maximum: usize,
    solver: Option<MorphInterpolationSolver>,
    derivatives: Option<[crate::Vector3; 2]>,
    mut evaluate: impl FnMut(Real, ParameterSide) -> Result<Point3, GeometryError>,
) -> Result<NurbsCurve, GeometryError> {
    let mut cache = HashMap::new();
    let mut point_at = |t: Real, side: ParameterSide| {
        let key = (t.to_bits(), side == ParameterSide::Left);
        if let Some(point) = cache.get(&key) {
            return Ok(*point);
        }
        let point = evaluate(t, side)?;
        cache.insert(key, point);
        Ok(point)
    };
    if source.is_rational() && source.degree() <= 3 {
        let fractions = error_fractions(ERROR_SAMPLES);
        if let Some(candidate) = rational::candidate(&mut point_at, source, maximum, solver)?
            && derivatives.is_none_or(|d| endpoint_derivatives_match(&candidate, d))
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
    fit_samples(
        &mut point_at,
        breaks,
        tolerance,
        maximum,
        &[],
        solver,
        derivatives,
    )
}

fn fit_samples(
    point_at: &mut impl FnMut(Real, ParameterSide) -> Result<Point3, GeometryError>,
    mut breaks: Vec<Break>,
    tolerance: Tolerance,
    maximum: usize,
    stations: &[Real],
    solver: Option<MorphInterpolationSolver>,
    derivatives: Option<[crate::Vector3; 2]>,
) -> Result<NurbsCurve, GeometryError> {
    let fractions = error_fractions(ERROR_SAMPLES);
    loop {
        let count = control_count(&breaks);
        if count > maximum {
            return Err(GeometryError::TooManyMorphCurveControlPoints { maximum });
        }
        let knots = knots(&breaks);
        let approximation = interpolate(point_at, knots, solver, derivatives)?;
        let validate::Errors {
            deviation,
            mut refinements,
        } = validate::errors(
            point_at,
            &approximation,
            &breaks,
            &fractions,
            tolerance.absolute() * 0.8,
        )?;
        let mut deviation = deviation;
        for &t in stations {
            for side in [ParameterSide::Left, ParameterSide::Right] {
                let error =
                    point_at(t, side)?.distance_to(approximation.evaluate_on_side(t, side)?)?;
                deviation = deviation.max(error);
                if error > tolerance.absolute() * 0.8 {
                    let index = breaks
                        .partition_point(|b| b.parameter <= t)
                        .saturating_sub(1)
                        .min(breaks.len() - 2);
                    if let Some(existing) = refinements.iter_mut().find(|(i, _)| *i == index) {
                        existing.1 = existing.1.max(error);
                    } else {
                        refinements.push((index, error));
                    }
                }
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
    solver: Option<MorphInterpolationSolver>,
    derivatives: Option<[crate::Vector3; 2]>,
) -> Result<NurbsCurve, GeometryError> {
    let mut axis = Axis::for_solver(3, knots, solver)?;
    let count = axis.stations.len();
    let mut targets = Vec::with_capacity(count);
    for station in &axis.stations {
        targets.push(point_at(station.parameter, station.side)?);
    }
    if let Some([first, last]) = derivatives {
        let span = axis.knots[4] - axis.knots[3];
        targets[1] = targets[0].translated(first.scaled(span / 3.)?)?;
        let span = axis.knots[count] - axis.knots[count - 1];
        targets[count - 2] = targets[count - 1].translated(last.scaled(-span / 3.)?)?;
        axis.fix_control(1);
        axis.fix_control(count - 2);
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
    let solution = axis.solve_with(rhs, 3, solver)?;
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

fn endpoint_derivatives_match(curve: &NurbsCurve, expected: [crate::Vector3; 2]) -> bool {
    [*curve.domain().start(), *curve.domain().end()]
        .into_iter()
        .zip(expected)
        .all(|(t, expected)| {
            curve
                .derivative_at(t)
                .and_then(|actual| {
                    let gap = crate::Vector3::try_from(std::array::from_fn(|i| {
                        actual.to_array()[i] - expected.to_array()[i]
                    }))?
                    .length()?;
                    Ok(gap <= 2048. * Real::EPSILON * expected.length()?)
                })
                .unwrap_or(false)
        })
}
