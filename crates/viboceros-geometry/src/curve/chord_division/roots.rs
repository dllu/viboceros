//! Ordered Bernstein root search with exact interval bounds and finite work.
use super::*;
use crate::exact_scalar::scalar;

const WORK_LIMIT: usize = 33_554_432;

pub(super) fn first_root(
    coefficients: &[Rational],
    interval: [Real; 2],
    after: Option<Real>,
) -> Result<Option<Real>, GeometryError> {
    let after = after.map(rational);
    let mut pending = vec![(coefficients.to_vec(), interval.map(rational))];
    let mut work = 0usize;
    while let Some((c, [start, end])) = pending.pop() {
        if after.as_ref().is_some_and(|t| end <= *t) {
            continue;
        }
        if c[0].is_zero() && after.as_ref().is_none_or(|t| start > *t) {
            return project_root(&start, after.as_ref()).map(Some);
        }
        let changes = variations(c.iter());
        if changes == 0 {
            if c.last().unwrap().is_zero() {
                return project_root(&end, after.as_ref()).map(Some);
            }
            continue;
        }
        let middle = (&start + &end) / BigInt::from(2);
        let [a, b, m] = [scalar(&start)?, scalar(&end)?, scalar(&middle)?];
        if m <= a || m >= b {
            // One variation certifies one real root. With multiple variations,
            // a nearly touching complex pair must not become a fake station.
            if changes == 1 || has_real_root(&c)? {
                return project_root(&middle, after.as_ref()).map(Some);
            }
            continue;
        }
        work = work
            .checked_add(
                c.len()
                    .checked_mul(c.len())
                    .ok_or(GeometryError::BezierDecompositionLimit)?,
            )
            .filter(|n| *n <= WORK_LIMIT)
            .ok_or(GeometryError::BezierDecompositionLimit)?;
        let n = c.len() - 1;
        let mut row = c;
        let mut left = vec![row[0].clone()];
        let mut right = vec![row[n].clone()];
        for count in (1..=n).rev() {
            for i in 0..count {
                row[i] = (&row[i] + &row[i + 1]) / BigInt::from(2);
            }
            left.push(row[0].clone());
            right.push(row[count - 1].clone());
        }
        right.reverse();
        pending.push((right, [middle.clone(), end]));
        pending.push((left, [start, middle]));
    }
    Ok(None)
}

fn project_root(t: &Rational, after: Option<&Rational>) -> Result<Real, GeometryError> {
    let parameter = scalar(t)?;
    if after.is_some_and(|previous| rational(parameter) <= *previous) {
        // The next station cannot be represented by a strictly forward
        // binary64 parameter. Reject instead of skipping it or looping.
        return Err(GeometryError::BezierDecompositionLimit);
    }
    Ok(parameter)
}

fn variations<'a>(values: impl IntoIterator<Item = &'a Rational>) -> usize {
    let mut previous = 0;
    let mut changes = 0;
    for value in values {
        let sign = if value.is_positive() {
            1
        } else if value.is_negative() {
            -1
        } else {
            0
        };
        if sign != 0 {
            changes += usize::from(previous != 0 && previous != sign);
            previous = sign;
        }
    }
    changes
}

/// Sturm's sequence counts distinct real roots of a rational polynomial in
/// (0,1), including repeated contacts. End roots are handled separately.
fn has_real_root(bernstein: &[Rational]) -> Result<bool, GeometryError> {
    if bernstein.len() > 129 {
        return Err(GeometryError::BezierDecompositionLimit);
    }
    let n = bernstein.len() - 1;
    let combinations = binomial(n);
    let mut polynomial = vec![Rational::zero(); n + 1];
    for (i, coefficient) in bernstein.iter().enumerate() {
        for (j, b) in binomial(n - i).into_iter().enumerate() {
            let term = coefficient * Rational::from_integer(&combinations[i] * b);
            polynomial[i + j] += if j % 2 == 0 { term } else { -term };
        }
    }
    trim(&mut polynomial);
    if polynomial.len() <= 1 {
        return Ok(false);
    }
    let derivative = polynomial
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, p)| p * BigInt::from(i))
        .collect::<Vec<_>>();
    let mut sequence = vec![polynomial, derivative];
    loop {
        let count = sequence.len();
        let a = &sequence[count - 2];
        let b = &sequence[count - 1];
        let mut remainder = a.clone();
        while remainder.len() >= b.len() && !remainder.is_empty() {
            let offset = remainder.len() - b.len();
            let scale = remainder.last().unwrap() / b.last().unwrap();
            for (i, coefficient) in b.iter().enumerate() {
                remainder[offset + i] -= &scale * coefficient;
            }
            trim(&mut remainder);
        }
        if remainder.is_empty() {
            break;
        }
        remainder.iter_mut().for_each(|p| *p = -p.clone());
        sequence.push(remainder);
    }
    let start = sequence.iter().map(|p| p[0].clone()).collect::<Vec<_>>();
    let end = sequence
        .iter()
        .map(|p| p.iter().sum::<Rational>())
        .collect::<Vec<_>>();
    Ok(variations(&start) > variations(&end))
}

fn trim(polynomial: &mut Vec<Rational>) {
    while polynomial.last().is_some_and(Zero::is_zero) {
        polynomial.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn non_dyadic_double_root_is_distinguished_from_a_nearly_touching_complex_pair() {
        let contact = [4., -6., 9.].map(rational);
        assert!((first_root(&contact, [0., 1.], Some(0.)).unwrap().unwrap() - 0.4).abs() < 1e-15);
        let epsilon = Rational::new(BigInt::from(1), BigInt::from(1) << 200);
        let miss = contact.map(|c| c + &epsilon);
        assert_eq!(first_root(&miss, [0., 1.], Some(0.)).unwrap(), None);
    }
}
