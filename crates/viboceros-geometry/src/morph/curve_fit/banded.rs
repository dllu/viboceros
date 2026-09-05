//! Cubic Greville collocation has at most two off-diagonals on either side.
//! For a valid nonsingular B-spline collocation matrix, total nonnegativity
//! permits elimination without pivoting (de Boor/Pinkus). This is deliberately
//! not a general-purpose banded solver: malformed or singular rows are rejected.

use crate::{GeometryError, Real, require_finite};
use faer::Mat;

pub(super) fn solve(rows: &[[Real; 5]], mut rhs: Mat<Real>) -> Result<Mat<Real>, GeometryError> {
    debug_assert_eq!(rhs.nrows(), rows.len());
    let count = rows.len();
    let mut bands = rows.to_vec();
    for pivot in 0..count {
        let diagonal = bands[pivot][2];
        if !diagonal.is_finite() || diagonal <= 0.0 {
            return Err(GeometryError::Degenerate {
                context: "cubic morph collocation pivot",
            });
        }
        for row in pivot + 1..(pivot + 3).min(count) {
            let multiplier = bands[row][pivot + 2 - row] / diagonal;
            require_finite([multiplier], "cubic morph collocation multiplier")?;
            bands[row][pivot + 2 - row] = 0.0;
            for column in pivot + 1..(pivot + 3).min(count) {
                let index = column + 2 - row;
                bands[row][index] =
                    (-multiplier).mul_add(bands[pivot][column + 2 - pivot], bands[row][index]);
            }
            for column in 0..rhs.ncols() {
                rhs[(row, column)] =
                    (-multiplier).mul_add(rhs[(pivot, column)], rhs[(row, column)]);
            }
        }
    }
    for row in (0..count).rev() {
        for column in 0..rhs.ncols() {
            let mut value = rhs[(row, column)];
            for next in row + 1..(row + 3).min(count) {
                value = (-bands[row][next + 2 - row]).mul_add(rhs[(next, column)], value);
            }
            let value = value / bands[row][2];
            require_finite([value], "cubic morph collocation solution")?;
            rhs[(row, column)] = value;
        }
    }
    Ok(rhs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::prelude::*;

    #[test]
    fn banded_solution_agrees_with_faer_full_pivot_on_nonuniform_repeated_knots() {
        for variant in 0..12 {
            let mut knots = vec![0.0; 4];
            for i in 1..24 {
                let t = (i as Real / 24.0).powi(1 + variant % 4);
                knots.extend(std::iter::repeat_n(t, 1 + (i + variant as usize) % 4));
            }
            knots.extend([1.0; 4]);
            let count = knots.len() - 4;
            let rows = (0..count)
                .map(|i| super::super::collocation_row(&knots, i).unwrap().0)
                .collect::<Vec<_>>();
            let matrix = Mat::from_fn(count, count, |i, j| {
                if i.abs_diff(j) <= 2 {
                    rows[i][j + 2 - i]
                } else {
                    0.0
                }
            });
            let expected = Mat::from_fn(count, 3, |i, j| ((i * 17 + j * 11) as Real).sin());
            let rhs = &matrix * &expected;
            let reference = matrix.full_piv_lu().solve(&rhs);
            let actual = solve(&rows, rhs).unwrap();
            for i in 0..count {
                for j in 0..3 {
                    assert!((actual[(i, j)] - reference[(i, j)]).abs() < 2e-12);
                    assert!((actual[(i, j)] - expected[(i, j)]).abs() < 2e-12);
                }
            }
        }
    }

    #[test]
    fn rejects_a_singular_interpolation_system() {
        assert!(solve(&[[0.0; 5]], Mat::zeros(1, 3)).is_err());
    }
}
