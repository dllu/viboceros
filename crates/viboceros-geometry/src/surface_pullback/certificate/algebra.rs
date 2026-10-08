//! Exact Bernstein products and outward rational Euclidean bounds.
use super::*;
use num_bigint::BigInt;

pub(super) fn binomial(n: usize, k: usize) -> BigInt {
    (0..k.min(n - k)).fold(BigInt::one(), |c, i| c * (n - i) / (i + 1))
}
pub(super) fn multiply(
    a: &[Rational],
    b: &[Rational],
    budget: &mut Budget,
) -> Result<Vec<Rational>, GeometryError> {
    budget.charge(a.len() * b.len())?;
    let (p, q) = (a.len() - 1, b.len() - 1);
    let mut output = vec![Rational::zero(); p + q + 1];
    for (i, a) in a.iter().enumerate() {
        for (j, b) in b.iter().enumerate() {
            let c = Rational::new(binomial(p, i) * binomial(q, j), binomial(p + q, i + j));
            output[i + j] += c * a * b;
            budget.check(&output[i + j])?;
        }
    }
    Ok(output)
}
pub(super) fn basis(
    a: &[Rational],
    b: &[Rational],
    degree: usize,
    budget: &mut Budget,
) -> Result<Vec<Vec<Rational>>, GeometryError> {
    let mut ap = vec![vec![Rational::one()]];
    let mut bp = ap.clone();
    for i in 1..=degree {
        ap.push(multiply(&ap[i - 1], a, budget)?);
        bp.push(multiply(&bp[i - 1], b, budget)?);
    }
    (0..=degree)
        .map(|i| {
            let mut basis = multiply(&ap[i], &bp[degree - i], budget)?;
            let scale = Rational::from_integer(binomial(degree, i));
            for r in &mut basis {
                *r *= &scale;
                budget.check(r)?;
            }
            Ok(basis)
        })
        .collect()
}
pub(super) fn difference(a: &[H], b: &[H], budget: &mut Budget) -> Result<Vec<H>, GeometryError> {
    budget.charge(4 * a.len() * b.len())?;
    let (p, q) = (a.len() - 1, b.len() - 1);
    let mut output = vec![std::array::from_fn(|_| Rational::zero()); p + q + 1];
    for (i, a) in a.iter().enumerate() {
        for (j, b) in b.iter().enumerate() {
            let c = Rational::new(binomial(p, i) * binomial(q, j), binomial(p + q, i + j));
            for axis in 0..3 {
                output[i + j][axis] += &c * (&a[axis] * &b[3] - &b[axis] * &a[3]);
            }
            output[i + j][3] += c * &a[3] * &b[3];
            for r in &output[i + j] {
                budget.check(r)?;
            }
        }
    }
    Ok(output)
}
pub(super) fn norm_bound(p: &H, limit: Real) -> Option<Real> {
    let square: Rational = p[..3].iter().map(|x| x * x).sum::<Rational>() / (&p[3] * &p[3]);
    let r = rational(limit);
    if square > &r * &r {
        return None;
    }
    let mut candidate = scalar(&(&p[0] / &p[3]))
        .unwrap_or(limit)
        .hypot(scalar(&(&p[1] / &p[3])).unwrap_or(limit))
        .hypot(scalar(&(&p[2] / &p[3])).unwrap_or(limit))
        .min(limit);
    for _ in 0..4 {
        let c = rational(candidate);
        if &c * &c >= square {
            return Some(candidate);
        }
        candidate = candidate.next_up().min(limit);
    }
    Some(limit)
}
pub(super) fn hull_bound(
    net: Vec<H>,
    limit: Real,
    budget: &mut Budget,
) -> Result<Option<Real>, GeometryError> {
    let mut pending = vec![(net, 0)];
    let mut answer = 0_f64;
    while let Some((net, depth)) = pending.pop() {
        budget.charge(4 * net.len())?;
        if norm_bound(&net[0], limit).is_none() || norm_bound(net.last().unwrap(), limit).is_none()
        {
            return Ok(None);
        }
        if let Some(upper) = net
            .iter()
            .try_fold(0_f64, |v, p| Some(v.max(norm_bound(p, limit)?)))
        {
            answer = answer.max(upper);
            continue;
        }
        if depth == MAX_DEPTH {
            return Ok(None);
        }
        let (a, b) = split(&net, budget)?;
        pending.push((b, depth + 1));
        pending.push((a, depth + 1));
    }
    Ok(Some(answer))
}
pub(super) fn split<const D: usize>(
    net: &[[Rational; D]],
    budget: &mut Budget,
) -> Result<(Net<D>, Net<D>), GeometryError> {
    budget.charge(D * net.len().pow(2))?;
    let mut work = net.to_vec();
    let mut left = vec![work[0].clone()];
    let mut right = vec![work.last().unwrap().clone()];
    for remaining in (1..work.len()).rev() {
        for i in 0..remaining {
            work[i] = std::array::from_fn(|j| (&work[i][j] + &work[i + 1][j]) / rational(2.));
            for r in &work[i] {
                budget.check(r)?;
            }
        }
        left.push(work[0].clone());
        right.push(work[remaining - 1].clone());
    }
    right.reverse();
    Ok((left, right))
}
