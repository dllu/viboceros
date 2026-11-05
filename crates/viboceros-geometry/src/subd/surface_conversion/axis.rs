//! Natural and periodic cubic station systems, one factorization per direction.
use super::*;
pub(super) use crate::cubic_grid::{apply_axis, solve_axis};
use crate::nurbs::{control_polygon_range, stable_knot_mean};

pub(super) fn parameters(
    surface: &NurbsSurface,
    axis: usize,
    closed: bool,
    singular: [bool; 2],
) -> Result<Vec<f64>, GeometryError> {
    let (degree, n, knots, periodic) = if axis == 0 {
        (
            surface.degree_u(),
            surface.control_point_count_u(),
            surface.knots_u(),
            surface.is_periodic_u(),
        )
    } else {
        (
            surface.degree_v(),
            surface.control_point_count_v(),
            surface.knots_v(),
            surface.is_periodic_v(),
        )
    };
    let (start, mut end) = control_polygon_range(degree, n, knots, periodic);
    if closed {
        end -= 1;
    }
    let values = (start..end)
        .map(|i| stable_knot_mean(&knots[i + 1..=i + degree]))
        .collect::<Result<Vec<_>, _>>()?;
    if !closed && degree == 3 {
        // Full-multiplicity joins are independent natural end conditions.
        // Keep their shared station, omitting only each natural piece's handles.
        let mut boundaries = vec![0];
        let mut i = degree + 1;
        while i < n {
            let first = i;
            let knot = knots[i];
            while i < knots.len() && knots[i] == knot {
                i += 1;
            }
            if i - first == degree {
                boundaries.push(first - 1);
            }
        }
        boundaries.push(n - 1);
        let mut omit = vec![false; n];
        for piece in boundaries.windows(2) {
            let [first, last] = [piece[0], piece[1]];
            if natural_piece(surface, axis, knots, first, last) {
                omit[first + 1] = first > 0 || !singular[0];
                omit[last - 1] = last + 1 < n || !singular[1];
            }
        }
        if omit.iter().any(|&v| v) {
            return Ok(values
                .into_iter()
                .enumerate()
                .filter_map(|(i, p)| (!omit[start + i]).then_some(p))
                .collect());
        }
    }
    if !closed && n == degree + 1 && n > 2 && linear_axis(surface, axis, &values) {
        return Ok(vec![values[0], *values.last().unwrap()]);
    }
    Ok(values)
}
// ON_ZERO_TOLERANCE, used for geometric simplification independently of document tolerance.
const ZERO_TOLERANCE: f64 = 1. / 4_294_967_296.;
fn natural_piece(
    surface: &NurbsSurface,
    axis: usize,
    knots: &[f64],
    first: usize,
    last: usize,
) -> bool {
    let n = last - first + 1;
    if n < 4 {
        return false;
    }
    let spans = &knots[first + 3..=last + 1];
    let step = spans[1] - spans[0];
    if step <= 0.
        || spans
            .windows(2)
            .any(|w| ((w[1] - w[0]) - step).abs() > 64. * f64::EPSILON * step.abs())
    {
        return false;
    }
    let u = surface.control_point_count_u();
    let v = surface.control_point_count_v();
    let index = |i, j| {
        let i = i + first;
        if axis == 0 { j * u + i } else { i * u + j }
    };
    let rows = if axis == 0 { v } else { u };
    let controls = surface.control_points();
    for j in 0..rows {
        if !(0..n).all(|i| controls[index(i, j)].weight() == controls[index(0, j)].weight()) {
            return false;
        }
        for end in [false, true] {
            let start = if end { n - 3 } else { 0 };
            let p = std::array::from_fn::<_, 3, _>(|i| {
                controls[index(start + i, j)].point().to_array()
            });
            let q = std::array::from_fn::<_, 3, _>(|c| {
                if n == 4 {
                    (p[0][c] + p[2][c]) * 0.5
                } else if end {
                    (p[0][c] + 2. * p[2][c]) / 3.
                } else {
                    (2. * p[0][c] + p[2][c]) / 3.
                }
            });
            if (0..3).any(|c| (p[1][c] - q[c]).abs() > ZERO_TOLERANCE) {
                return false;
            }
        }
    }
    true
}
fn linear_axis(surface: &NurbsSurface, axis: usize, greville: &[f64]) -> bool {
    let points = surface.control_points();
    if points.iter().any(|p| p.weight() != points[0].weight()) {
        return false;
    }
    let u = surface.control_point_count_u();
    let v = surface.control_point_count_v();
    let extent = greville[greville.len() - 1] - greville[0];
    if !extent.is_finite() || extent <= 0. {
        return false;
    }
    let index = |i, j| if axis == 0 { j * u + i } else { i * u + j };
    let rows = if axis == 0 { v } else { u };
    let n = greville.len();
    for j in 0..rows {
        if !(0..n).all(|i| points[index(i, j)].weight() == points[index(0, j)].weight()) {
            return false;
        }
        let a = points[index(0, j)].point().to_array();
        let b = points[index(n - 1, j)].point().to_array();
        for (i, &g) in greville.iter().enumerate() {
            let t = (g - greville[0]) / extent;
            let p = points[index(i, j)].point().to_array();
            for c in 0..3 {
                let expected = a[c] * (1. - t) + b[c] * t;
                if (p[c] - expected).abs()
                    > 64.
                        * f64::EPSILON
                        * a[c]
                            .abs()
                            .max(b[c].abs())
                            .max((b[c] - a[c]).abs())
                            .max(f64::MIN_POSITIVE)
                        + ZERO_TOLERANCE
                {
                    return false;
                }
            }
        }
    }
    true
}
