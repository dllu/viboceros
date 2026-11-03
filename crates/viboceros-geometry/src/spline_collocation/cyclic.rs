//! Greville cyclic bands with a small dense border, avoiding an n² matrix.
use super::*;
use faer::prelude::*;

pub(crate) fn solve(
    degree: usize,
    rows: &[(usize, Vec<Real>)],
    shift: usize,
    rhs: Mat<Real>,
) -> Result<Mat<Real>, GeometryError> {
    let n = rows.len();
    let channels = rhs.ncols();
    if degree == 0
        || degree > n
        || shift >= n
        || rhs.nrows() != n
        || n.saturating_mul(3 * degree + channels + 1) > 8 * 1024 * 1024
    {
        return Err(GeometryError::PeriodicInterpolationSolveFailed);
    }
    // Rotate columns so Greville row i is centered on control i. Cutting the
    // last p rows/columns leaves a noncyclic totally nonnegative band A.
    let rows = rows
        .iter()
        .map(|(first, values)| {
            values
                .iter()
                .enumerate()
                .map(|(j, &v)| (((first + j) % n + n - shift) % n, v))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let main = n - degree;
    let mut targets = Mat::zeros(main, channels + degree);
    let mut bands = Vec::with_capacity(main);
    for i in 0..main {
        for c in 0..channels {
            targets[(i, c)] = rhs[(i, c)];
        }
        let first = i.saturating_sub(degree);
        let mut values = vec![0.; (i + degree + 1).min(main) - first];
        for &(j, v) in &rows[i] {
            if v == 0. {
                continue;
            }
            if j >= main {
                targets[(i, channels + j - main)] -= v;
            } else if j >= first && j < first + values.len() {
                values[j - first] += v;
            } else {
                return Err(GeometryError::PeriodicInterpolationSolveFailed);
            }
        }
        bands.push((first, values));
    }
    // A [x X] = [b -B]; (D + C X) y = d - C x; result = [x + X y; y].
    let solved = super::general_banded::solve(degree, &bands, targets)?;
    let mut schur = Mat::zeros(degree, degree);
    let mut targets = Mat::zeros(degree, channels);
    for i in 0..degree {
        for c in 0..channels {
            targets[(i, c)] = rhs[(main + i, c)];
        }
        for &(j, v) in &rows[main + i] {
            if j >= main {
                schur[(i, j - main)] += v;
            } else {
                for k in 0..degree {
                    schur[(i, k)] = v.mul_add(solved[(j, channels + k)], schur[(i, k)]);
                }
                for c in 0..channels {
                    targets[(i, c)] = (-v).mul_add(solved[(j, c)], targets[(i, c)]);
                }
            }
        }
    }
    let border = schur.full_piv_lu().solve(&targets);
    let result = Mat::from_fn(n, channels, |i, c| {
        let i = (i + n - shift) % n;
        if i >= main {
            border[(i - main, c)]
        } else {
            (0..degree).fold(solved[(i, c)], |v, k| {
                solved[(i, channels + k)].mul_add(border[(k, c)], v)
            })
        }
    });
    require_finite(
        (0..n)
            .flat_map(|i| (0..channels).map(move |c| (i, c)))
            .map(|(i, c)| result[(i, c)]),
        "cyclic Greville solution",
    )?;
    Ok(result)
}
