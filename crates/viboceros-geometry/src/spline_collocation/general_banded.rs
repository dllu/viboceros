//! Bounded nonpivoting solve for totally nonnegative Greville collocation.
use super::*;

pub(crate) fn solve(
    degree: usize,
    rows: &[(usize, Vec<Real>)],
    mut rhs: Mat<Real>,
) -> Result<Mat<Real>, GeometryError> {
    let count = rows.len();
    let width = degree
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(invalid)?;
    let entries = count
        .checked_mul(width)
        .filter(|&n| n <= 8 * 1024 * 1024)
        .ok_or_else(invalid)?;
    if rhs.nrows() != count {
        return Err(invalid());
    }
    let mut bands = vec![0.; entries];
    for (i, (first, values)) in rows.iter().enumerate() {
        for (j, &v) in values.iter().enumerate() {
            let column = first + j;
            if column >= count || column + degree < i || column > i + degree {
                return Err(invalid());
            }
            bands[i * width + column + degree - i] = v;
        }
    }
    for pivot in 0..count {
        let diagonal = bands[pivot * width + degree];
        if !diagonal.is_finite() || diagonal <= 0. {
            return Err(invalid());
        }
        for row in pivot + 1..(pivot + degree + 1).min(count) {
            let index = row * width + pivot + degree - row;
            let multiplier = bands[index] / diagonal;
            bands[index] = 0.;
            for column in pivot + 1..(pivot + degree + 1).min(count) {
                let index = row * width + column + degree - row;
                bands[index] = (-multiplier)
                    .mul_add(bands[pivot * width + column + degree - pivot], bands[index]);
            }
            for c in 0..rhs.ncols() {
                rhs[(row, c)] = (-multiplier).mul_add(rhs[(pivot, c)], rhs[(row, c)]);
            }
        }
    }
    for row in (0..count).rev() {
        for c in 0..rhs.ncols() {
            let mut value = rhs[(row, c)];
            for next in row + 1..(row + degree + 1).min(count) {
                value = (-bands[row * width + next + degree - row]).mul_add(rhs[(next, c)], value);
            }
            let value = value / bands[row * width + degree];
            require_finite([value], "Greville collocation solution")?;
            rhs[(row, c)] = value;
        }
    }
    Ok(rhs)
}
fn invalid() -> GeometryError {
    GeometryError::InvalidControlNet {
        context: "invalid or singular banded Greville system",
    }
}
