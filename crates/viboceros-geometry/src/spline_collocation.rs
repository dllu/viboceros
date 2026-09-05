//! Shared adaptive cubic knots and sided curve/tensor-axis collocation.

use crate::{GeometryError, ParameterSide, Real, nurbs::bspline_basis_values, require_finite};
use faer::Mat;
use std::ops::RangeInclusive;

mod axis;
mod banded;
pub(crate) use axis::Axis;

pub(crate) const DEGREE: usize = 3;

#[derive(Clone, Copy)]
pub(crate) struct Break {
    pub parameter: Real,
    pub multiplicity: usize,
}

pub(crate) fn seed_breaks(
    degree: usize,
    knots: &[Real],
    domain: RangeInclusive<Real>,
) -> Vec<Break> {
    let mut breaks = vec![Break {
        parameter: *domain.start(),
        multiplicity: DEGREE + 1,
    }];
    for group in knots.chunk_by(|a, b| a == b) {
        if group[0] > *domain.start() && group[0] < *domain.end() {
            breaks.push(Break {
                parameter: group[0],
                multiplicity: (DEGREE + group.len())
                    .saturating_sub(degree)
                    .clamp(1, DEGREE + 1),
            });
        }
    }
    breaks.push(Break {
        parameter: *domain.end(),
        multiplicity: DEGREE + 1,
    });
    breaks
}

pub(crate) fn control_count(breaks: &[Break]) -> usize {
    breaks.iter().map(|b| b.multiplicity).sum::<usize>() - DEGREE - 1
}

pub(crate) fn knots(breaks: &[Break]) -> Vec<Real> {
    breaks
        .iter()
        .flat_map(|b| std::iter::repeat_n(b.parameter, b.multiplicity))
        .collect()
}

pub(crate) fn error_fractions(steps: usize) -> Vec<Real> {
    (1..steps)
        .flat_map(|i| {
            let fraction = i as Real / steps as Real;
            [
                fraction,
                0.5 * (1.0 - (std::f64::consts::PI * fraction).cos()),
            ]
        })
        .chain([0.0, 1.0])
        .collect()
}

pub(crate) fn solve(rows: &[[Real; 5]], rhs: Mat<Real>) -> Result<Mat<Real>, GeometryError> {
    banded::solve(rows, rhs)
}

pub(crate) fn collocation_row(
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

pub(crate) fn stable_lerp(start: Real, end: Real, fraction: Real) -> Result<Real, GeometryError> {
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
