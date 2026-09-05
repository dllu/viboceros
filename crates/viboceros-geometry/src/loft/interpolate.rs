use super::*;
use crate::{
    Point3,
    nurbs::{bspline_basis_values, clamped_uniform_knots},
    require_finite,
};
use faer::{Mat, prelude::*};

pub(super) fn loft(
    sections: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
) -> Result<(usize, Vec<Real>, Vec<WeightedPoint3>), GeometryError> {
    let n = sections.len();
    let width = sections[0].control_points().len();
    let intervals = section_intervals(sections, style, closed)?;
    let parameters = cumulative(&intervals)?;
    if matches!(style, LoftStyle::Loose | LoftStyle::Straight) {
        let degree = if style == LoftStyle::Straight {
            1
        } else if closed {
            3
        } else {
            3.min(n - 1)
        };
        let count = n + if closed { degree } else { 0 };
        let knots = if style == LoftStyle::Loose {
            if closed {
                periodic_knots(&vec![1.0; n], degree)?
            } else {
                clamped_uniform_knots(degree, count)?
            }
        } else {
            let mut knots = vec![0.0];
            knots.extend_from_slice(&parameters);
            knots.push(*parameters.last().unwrap());
            knots
        };
        let controls = (0..width)
            .flat_map(|v| {
                (0..count).map(move |u| {
                    let shift = if closed && style == LoftStyle::Loose {
                        n - 1
                    } else {
                        0
                    };
                    sections[(u + shift) % n].control_points()[v]
                })
            })
            .collect();
        return Ok((degree, knots, controls));
    }
    let knots = if closed {
        periodic_knots(&intervals, 3)?
    } else {
        let mut knots = vec![0.0; 4];
        knots.extend_from_slice(&parameters[1..n - 1]);
        knots.extend(std::iter::repeat_n(parameters[n - 1], 4));
        knots
    };
    let unknowns = n + if closed { 0 } else { 2 };
    let count = n + if closed { 3 } else { 2 };
    let mut matrix = Mat::zeros(unknowns, unknowns);
    for i in 0..n {
        for (j, value) in bspline_basis_values(&knots, 3, count, parameters[i])?
            .into_iter()
            .enumerate()
        {
            matrix[(i, if closed { j % n } else { j })] += value;
        }
    }
    if !closed {
        matrix[(n, 1)] = 1.0;
        matrix[(n + 1, count - 2)] = 1.0;
    }
    let weight_scale = sections
        .iter()
        .flat_map(|c| c.control_points())
        .map(|c| c.weight().abs())
        .fold(0.0, Real::max);
    let origin = sections[0]
        .control_points()
        .iter()
        .map(|p| p.point().to_array())
        .collect::<Vec<_>>();
    let data = Mat::from_fn(n, width * 4, |i, j| {
        let cp = sections[i].control_points()[j / 4];
        let w = cp.weight() / weight_scale;
        if j % 4 == 3 {
            w
        } else {
            (cp.point().to_array()[j % 4] - origin[j / 4][j % 4]) * w
        }
    });
    let start = (!closed)
        .then(|| end_coefficients(sections, weight_scale, false))
        .transpose()?;
    let end = (!closed)
        .then(|| end_coefficients(sections, weight_scale, true))
        .transpose()?;
    let rhs = Mat::from_fn(unknowns, width * 4, |i, j| {
        if i < n {
            data[(i, j)]
        } else if i == n {
            let [a, b] = start.unwrap();
            data[(0, j)] + a * (data[(1, j)] - data[(0, j)])
                - b * (data[(2.min(n - 1), j)] - data[(1, j)])
        } else {
            let [a, b] = end.unwrap();
            data[(n - 1, j)] + a * (data[(n - 2, j)] - data[(n - 1, j)])
                - b * (data[(n.saturating_sub(3), j)] - data[(n - 2, j)])
        }
    });
    require_finite(
        (0..rhs.ncols())
            .flat_map(|j| (0..rhs.nrows()).map(move |i| (i, j)))
            .map(|(i, j)| rhs[(i, j)]),
        "loft interpolation targets",
    )?;
    let solution = matrix.full_piv_lu().solve(&rhs);
    let mut controls = Vec::with_capacity(count * width);
    for v in 0..width {
        // A constant homogeneous channel is exactly representable. Retaining
        // it avoids introducing spurious rationality or world-offset variation.
        let fixed: [Option<Real>; 4] = std::array::from_fn(|j| {
            let first = data[(0, v * 4 + j)];
            (0..n)
                .all(|i| data[(i, v * 4 + j)] == first)
                .then_some(first)
        });
        for u in 0..count {
            if !closed && (u == 0 || u == count - 1) {
                controls.push(sections[if u == 0 { 0 } else { n - 1 }].control_points()[v]);
                continue;
            }
            let c: [Real; 4] = std::array::from_fn(|j| {
                fixed[j].unwrap_or(solution[(if closed { u % n } else { u }, v * 4 + j)])
            });
            require_finite(c, "loft homogeneous controls")?;
            let point = Point3::try_from(std::array::from_fn(|j| c[j] / c[3] + origin[v][j]))?;
            controls.push(WeightedPoint3::try_new(point, c[3] * weight_scale)?);
        }
    }
    Ok((3, knots, controls))
}

fn section_intervals(
    sections: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
) -> Result<Vec<Real>, GeometryError> {
    let n = sections.len();
    (0..n - usize::from(!closed))
        .map(|i| {
            let chord = sections[i]
                .control_points()
                .iter()
                .zip(sections[(i + 1) % n].control_points())
                .map(|(a, b)| a.point().distance_to(b.point()))
                .try_fold(0.0, |a: Real, b| Ok::<_, GeometryError>(a.max(b?)))?;
            if chord == 0.0 {
                return Err(GeometryError::Degenerate {
                    context: "coincident loft sections",
                });
            }
            Ok(match style {
                LoftStyle::Uniform | LoftStyle::Loose => 1.0,
                LoftStyle::Tight => chord.sqrt(),
                _ => chord,
            })
        })
        .collect()
}

fn cumulative(intervals: &[Real]) -> Result<Vec<Real>, GeometryError> {
    let mut parameters = vec![0.0];
    for interval in intervals {
        let next = parameters.last().unwrap() + interval;
        require_finite([next], "loft parameters")?;
        if next <= *parameters.last().unwrap() {
            return Err(GeometryError::InvalidCurveParameterInterval);
        }
        parameters.push(next);
    }
    Ok(parameters)
}

fn periodic_knots(intervals: &[Real], degree: usize) -> Result<Vec<Real>, GeometryError> {
    let mut before = vec![];
    let mut t = 0.0;
    for i in 0..degree {
        t -= intervals[intervals.len() - 1 - i % intervals.len()];
        before.push(t);
    }
    let mut knots = before.into_iter().rev().collect::<Vec<_>>();
    knots.extend(cumulative(intervals)?);
    t = *knots.last().unwrap();
    for i in 0..degree {
        t += intervals[i % intervals.len()];
        knots.push(t);
    }
    require_finite(knots.iter().copied(), "periodic loft knots")?;
    Ok(knots)
}

/// Automatic endpoint handles follow the parabola in the complete homogeneous
/// control space, independently of the max-Euclidean-chord section knot spacing.
fn end_coefficients(
    sections: &[NurbsCurve],
    weight_scale: Real,
    end: bool,
) -> Result<[Real; 2], GeometryError> {
    if sections.len() == 2 {
        return Ok([1.0 / 3.0, 0.0]);
    }
    let n = sections.len();
    let indices = if end {
        [n - 1, n - 2, n - 3]
    } else {
        [0, 1, 2]
    };
    let scale = indices
        .iter()
        .flat_map(|&i| sections[i].control_points())
        .flat_map(|c| c.point().to_array())
        .map(Real::abs)
        .fold(1.0, Real::max);
    let distance = |a: usize, b: usize| {
        sections[a]
            .control_points()
            .iter()
            .zip(sections[b].control_points())
            .fold(0.0_f64, |norm, (a, b)| {
                let wa = a.weight() / weight_scale;
                let wb = b.weight() / weight_scale;
                let norm = a
                    .point()
                    .to_array()
                    .into_iter()
                    .zip(b.point().to_array())
                    .fold(norm, |norm, (a, b)| {
                        // Preserve local differences for equal/nearby weights;
                        // subtracting independently scaled world coordinates
                        // needlessly loses translated section detail.
                        let delta = a - b;
                        let delta = if delta.is_finite() {
                            delta / scale
                        } else {
                            a / scale - b / scale
                        };
                        norm.hypot(delta.mul_add(wa, (b / scale) * (wa - wb)))
                    });
                norm.hypot((wa - wb) / scale)
            })
    };
    let a = distance(indices[0], indices[1]);
    let b = distance(indices[1], indices[2]);
    let scale = a.max(b);
    let (a, b) = (a / scale, b / scale);
    let result = [(2.0 * a + b) / (3.0 * (a + b)), a * a / (3.0 * b * (a + b))];
    require_finite(result, "loft endpoint handle")?;
    Ok(result)
}
