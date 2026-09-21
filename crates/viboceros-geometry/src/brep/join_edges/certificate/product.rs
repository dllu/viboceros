//! For positive rational curves, A-B = (Na Wb - Nb Wa)/(Wa Wb).
//! Exact Bernstein multiplication makes this another positive rational curve;
//! its projected control hull bounds the entire difference, not sampled points.
use super::*;
use crate::exact_scalar::{Rational, rational};
mod extract;
#[cfg(test)]
mod tests;

type H = [Rational; 4];
const MAX_DEGREE: usize = 16;
const MAX_DEPTH: usize = 16;

pub(super) fn bound(
    a: &NurbsCurve,
    b: &NurbsCurve,
    reversed: bool,
    limit: Real,
    tighten: bool,
    charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
) -> Result<Option<Real>, GeometryError> {
    if [a, b].iter().any(|c| c.degree() > MAX_DEGREE) {
        return Ok(None);
    }
    // Exact clamped endpoint rejection avoids rational algebra for most broad
    // phase false positives. Unclamped endpoints are evaluated by extraction.
    for end in [false, true] {
        if let (Some(ap), Some(bp)) = (endpoint(a, end), endpoint(b, end ^ reversed))
            && point_bound(ap, bp, limit).is_none()
        {
            return Ok(None);
        }
    }
    let Some(a) = extract::Spline::new(a, false, charge)? else {
        return Ok(None);
    };
    let Some(b) = extract::Spline::new(b, reversed, charge)? else {
        return Ok(None);
    };
    let (mut ai, mut bi) = (a.degree(), b.degree());
    let mut left = rational(0.);
    let one = rational(1.);
    let mut answer: Real = 0.;
    while left < one {
        while a.knots[ai + 1] <= left {
            ai += 1;
        }
        while b.knots[bi + 1] <= left {
            bi += 1;
        }
        let right = std::cmp::min(&a.knots[ai + 1], &b.knots[bi + 1]).clone();
        let ac = a.extract(ai, &left, &right, charge)?;
        let bc = b.extract(bi, &left, &right, charge)?;
        charge(4 * ac.len() * bc.len())?;
        let net = multiply(&ac, &bc);
        let Some(upper) = hull_bound(net, limit, tighten, charge)? else {
            return Ok(None);
        };
        answer = answer.max(upper);
        left = right;
    }
    Ok(Some(answer))
}

fn endpoint(curve: &NurbsCurve, end: bool) -> Option<Point3> {
    let p = curve.degree();
    let knots = curve.knots();
    let controls = curve.control_points();
    let (knots, t, index) = if end {
        (
            &knots[knots.len() - p - 1..],
            *curve.domain().end(),
            controls.len() - 1,
        )
    } else {
        (&knots[..=p], *curve.domain().start(), 0)
    };
    knots
        .iter()
        .all(|&k| k == t)
        .then(|| controls[index].point())
}

fn binomial(n: usize, k: usize) -> u64 {
    // n <= 2*MAX_DEGREE: even the intermediate integer fits comfortably.
    (0..k.min(n - k)).fold(1, |c, i| c * (n - i) as u64 / (i + 1) as u64)
}

fn multiply(a: &[H], b: &[H]) -> Vec<H> {
    let (p, q) = (a.len() - 1, b.len() - 1);
    let mut net: Vec<H> = (0..=p + q)
        .map(|_| std::array::from_fn(|_| rational(0.)))
        .collect();
    for (i, a) in a.iter().enumerate() {
        for (j, b) in b.iter().enumerate() {
            let c = Rational::new(
                (binomial(p, i) * binomial(q, j)).into(),
                binomial(p + q, i + j).into(),
            );
            for axis in 0..3 {
                net[i + j][axis] += &c * (&a[axis] * &b[3] - &b[axis] * &a[3]);
            }
            net[i + j][3] += c * &a[3] * &b[3];
        }
    }
    net
}

fn hull_bound(
    net: Vec<H>,
    limit: Real,
    tighten: bool,
    charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
) -> Result<Option<Real>, GeometryError> {
    let mut pending = vec![(net, 0)];
    let mut answer: Real = 0.;
    let mut lower: Real = 0.;
    let mut target = limit * 1e-12;
    while let Some((points, depth)) = pending.pop() {
        charge(points.len())?;
        // Endpoints belong to the curve: failure here really disproves this
        // correspondence. An interior control outside the ball does not.
        for p in [&points[0], &points[points.len() - 1]] {
            let Some(bound) = refine::norm_bound(p, limit) else {
                return Ok(None);
            };
            // This is only a refinement heuristic, never an acceptance test.
            lower = lower.max(bound.next_down().max(0.));
        }
        let upper = points
            .iter()
            .try_fold(0_f64, |u, p| Some(u.max(refine::norm_bound(p, limit)?)));
        if let Some(upper) = upper {
            if depth == 0 {
                target = upper * 1e-12;
            }
            if !tighten || depth == MAX_DEPTH || upper <= lower + target || upper <= answer {
                answer = answer.max(upper);
                continue;
            }
        } else if depth == MAX_DEPTH {
            // Exhaustion is inconclusive, not evidence of a match.
            return Ok(None);
        }
        charge(4 * points.len() * points.len())?;
        let (left, right) = refine::split(points);
        pending.push((right, depth + 1));
        pending.push((left, depth + 1));
    }
    Ok(Some(answer))
}
