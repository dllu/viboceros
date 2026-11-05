//! Sparse natural and periodic cubic collocation, shared by curves and tensors.
use crate::GeometryError;
use faer::prelude::*;
use faer::sparse::{SparseColMat, Triplet};

fn row(i: usize, n: usize, closed: bool, anchors: &[usize]) -> [(usize, f64); 3] {
    if !closed && (i == 0 || i == n - 1) || anchors.binary_search(&i).is_ok() {
        [(i, 1.), (i, 0.), (i, 0.)]
    } else {
        [
            ((i + n - 1) % n, 1. / 6.),
            (i, 4. / 6.),
            ((i + 1) % n, 1. / 6.),
        ]
    }
}
pub(crate) fn solve_axis(
    points: &[[f64; 3]],
    counts: [usize; 2],
    axis: usize,
    closed: bool,
    anchors: &[usize],
) -> Result<Vec<[f64; 3]>, GeometryError> {
    let n = counts[axis];
    let other = counts[1 - axis];
    let index = |i, j| {
        if axis == 0 {
            j * counts[0] + i
        } else {
            i * counts[0] + j
        }
    };
    let triplets = (0..n)
        .flat_map(|i| {
            row(i, n, closed, anchors)
                .into_iter()
                .filter(|&(_, w)| w != 0.)
                .map(move |(j, w)| Triplet::new(i, j, w))
        })
        .collect::<Vec<_>>();
    let matrix = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, &triplets)
        .map_err(|_| GeometryError::SubDInterpolationFailed)?;
    let factor = matrix
        .sp_lu()
        .map_err(|_| GeometryError::SubDInterpolationFailed)?;
    let rhs = faer::Mat::from_fn(n, other * 3, |i, j| points[index(i, j / 3)][j % 3]);
    let solved = factor.solve(&rhs);
    let mut result = vec![[0.; 3]; points.len()];
    for i in 0..n {
        for j in 0..other {
            result[index(i, j)] = std::array::from_fn(|c| solved[(i, j * 3 + c)]);
        }
    }
    Ok(result)
}
pub(crate) fn apply_axis(
    points: &[[f64; 3]],
    counts: [usize; 2],
    axis: usize,
    closed: bool,
    anchors: &[usize],
) -> Vec<[f64; 3]> {
    let n = counts[axis];
    let other = counts[1 - axis];
    let index = |i, j| {
        if axis == 0 {
            j * counts[0] + i
        } else {
            i * counts[0] + j
        }
    };
    let mut result = vec![[0.; 3]; points.len()];
    for i in 0..n {
        for j in 0..other {
            for (k, w) in row(i, n, closed, anchors) {
                for c in 0..3 {
                    result[index(i, j)][c] += points[index(k, j)][c] * w;
                }
            }
        }
    }
    result
}
