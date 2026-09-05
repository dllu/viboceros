use super::*;
use crate::{ParameterSide, nurbs::bspline_basis_values};
use faer::Mat;
use std::collections::HashMap;

mod banded;

#[cfg(test)]
mod tests;

const DEGREE: usize = 3;
const ERROR_SAMPLES: usize = 16;

#[derive(Clone, Copy)]
struct Break {
    parameter: Real,
    multiplicity: usize,
}

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
    let domain = source.domain();
    let mut breaks = vec![Break {
        parameter: *domain.start(),
        multiplicity: DEGREE + 1,
    }];
    for group in source.knots().chunk_by(|a, b| a == b) {
        if group[0] > *domain.start() && group[0] < *domain.end() {
            // Never smooth over a structural corner, acceleration break, or
            // positional jump. Smooth refinements retain simple cubic knots.
            breaks.push(Break {
                parameter: group[0],
                multiplicity: (DEGREE + group.len())
                    .saturating_sub(source.degree())
                    .clamp(1, DEGREE + 1),
            });
        }
    }
    breaks.push(Break {
        parameter: *domain.end(),
        multiplicity: DEGREE + 1,
    });
    let fractions = (1..ERROR_SAMPLES)
        .flat_map(|i| {
            let fraction = i as Real / ERROR_SAMPLES as Real;
            // An independent nonuniform grid reduces aliasing with both the
            // interpolation abscissae and the uniformly spaced validation grid.
            [
                fraction,
                0.5 * (1.0 - (std::f64::consts::PI * fraction).cos()),
            ]
        })
        .chain([0.0, 1.0])
        .collect::<Vec<_>>();
    loop {
        let count = breaks.iter().map(|b| b.multiplicity).sum::<usize>() - DEGREE - 1;
        if count > maximum {
            return Err(GeometryError::TooManyMorphCurveControlPoints { maximum });
        }
        let knots = breaks
            .iter()
            .flat_map(|b| std::iter::repeat_n(b.parameter, b.multiplicity))
            .collect::<Vec<_>>();
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
    let solution = banded::solve(&rows, rhs)?;
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

fn collocation_row(
    knots: &[Real],
    i: usize,
) -> Result<([Real; 5], Real, ParameterSide, bool), GeometryError> {
    let fixed = knots[i + 1] == knots[i + DEGREE];
    let t = if fixed {
        knots[i + 1]
    } else {
        stable_mean3(knots[i + 1], knots[i + 2], knots[i + 3])?
    };
    let side = if fixed && knots[i] < t {
        ParameterSide::Left
    } else {
        ParameterSide::Right
    };
    let mut row = [0.0; 5];
    if fixed {
        // Full-order knots have independent endpoint rows at the same t.
        row[2] = 1.0;
    } else {
        let values = bspline_basis_values(knots, DEGREE, knots.len() - DEGREE - 1, t)?;
        for (j, value) in values.into_iter().enumerate() {
            if i.abs_diff(j) <= 2 {
                row[j + 2 - i] = value;
            } else if value != 0.0 {
                return Err(GeometryError::Degenerate {
                    context: "cubic morph collocation bandwidth",
                });
            }
        }
    }
    Ok((row, t, side, fixed))
}

fn stable_mean3(first: Real, second: Real, third: Real) -> Result<Real, GeometryError> {
    let scale = first.abs().max(second.abs()).max(third.abs());
    if scale == 0.0 {
        return Ok(0.0);
    }
    let mean = (((first / scale + second / scale) + third / scale) / 3.0).clamp(-1.0, 1.0) * scale;
    require_finite([mean], "cubic morph Greville parameter")?;
    Ok(mean)
}

fn stable_lerp(start: Real, end: Real, fraction: Real) -> Result<Real, GeometryError> {
    let parameter = if fraction == 0.0 {
        start
    } else if fraction == 1.0 {
        end
    } else {
        start.mul_add(1.0 - fraction, end * fraction)
    };
    require_finite([parameter], "cubic morph sample parameter")?;
    Ok(parameter)
}
