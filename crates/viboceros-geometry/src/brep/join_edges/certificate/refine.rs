//! Positive rational Bernstein hulls bound a complete curve difference.
use super::*;
use crate::exact_scalar::{Rational, rational, scalar};

pub(in crate::brep::join_edges) fn refined_curve_bound(
    a: &NurbsCurve,
    b: &NurbsCurve,
    reversed: bool,
    limit: Real,
    mut charge: impl FnMut(usize) -> Result<(), GeometryError>,
) -> Result<Option<Real>, GeometryError> {
    let Some(initial) = curve_bound(a, b, reversed, limit) else {
        return product::bound(a, b, reversed, limit, true, &mut charge);
    };
    let n = a.control_points().len();
    if initial == 0.
        || a.degree() > 16
        || n != a.degree() + 1
        || common_basis_bound(a, b, reversed, limit).is_none()
        || !a.knots()[..n].iter().all(|k| *k == *a.domain().start())
        || !a.knots()[n..].iter().all(|k| *k == *a.domain().end())
    {
        return Ok(Some(initial));
    }
    let controls = a
        .control_points()
        .iter()
        .enumerate()
        .map(|(i, ac)| {
            let bc = &b.control_points()[if reversed { n - 1 - i } else { i }];
            let weight = rational(ac.weight().abs());
            let mut h: [Rational; 4] = std::array::from_fn(|axis| {
                if axis == 3 {
                    weight.clone()
                } else {
                    (rational(ac.point().to_array()[axis]) - rational(bc.point().to_array()[axis]))
                        * &weight
                }
            });
            // All weights have a common nonzero sign, proved by common_basis_bound.
            h[3] = weight;
            h
        })
        .collect::<Vec<_>>();
    let mut pending = vec![(controls, 0)];
    let mut answer: Real = 0.;
    let mut lower: Real = 0.;
    let target = initial * 1e-12;
    while let Some((points, depth)) = pending.pop() {
        charge(points.len())?;
        let mut upper: Real = 0.;
        for (i, p) in points.iter().enumerate() {
            let bound = norm_bound(p, initial)
                .ok_or_else(|| invalid("invalid rational difference hull"))?;
            upper = upper.max(bound);
            if i == 0 || i + 1 == points.len() {
                lower = lower.max(bound.next_down().max(0.));
            }
        }
        if depth == 16 || upper <= lower + target || upper <= answer {
            answer = answer.max(upper);
            continue;
        }
        charge(points.len().saturating_mul(points.len()))?;
        let (left, right) = split(points);
        pending.push((right, depth + 1));
        pending.push((left, depth + 1));
    }
    Ok(Some(answer.min(initial)))
}

pub(super) fn norm_bound(point: &[Rational; 4], limit: Real) -> Option<Real> {
    let coordinates: [Rational; 3] = std::array::from_fn(|i| &point[i] / &point[3]);
    let square: Rational = coordinates.iter().map(|x| x * x).sum();
    let r = rational(limit);
    if square > &r * &r {
        return None;
    }
    let mut bound = scalar(&coordinates[0])
        .unwrap_or(limit)
        .hypot(scalar(&coordinates[1]).unwrap_or(limit))
        .hypot(scalar(&coordinates[2]).unwrap_or(limit))
        .min(limit);
    for _ in 0..4 {
        let r = rational(bound);
        if &r * &r >= square {
            return Some(bound);
        }
        bound = bound.next_up().min(limit);
    }
    Some(limit)
}

pub(super) fn split(mut points: Vec<[Rational; 4]>) -> (Vec<[Rational; 4]>, Vec<[Rational; 4]>) {
    let n = points.len();
    let mut left = vec![points[0].clone()];
    let mut right = vec![points[n - 1].clone()];
    let half = rational(0.5);
    for remaining in (1..n).rev() {
        for i in 0..remaining {
            points[i] = std::array::from_fn(|j| (&points[i][j] + &points[i + 1][j]) * &half);
        }
        left.push(points[0].clone());
        right.push(points[remaining - 1].clone());
    }
    right.reverse();
    (left, right)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve(y: &[Real], weights: &[Real]) -> NurbsCurve {
        let n = y.len();
        NurbsCurve::try_new_rational(
            n - 1,
            y.iter()
                .zip(weights)
                .enumerate()
                .map(|(i, (&y, &w))| {
                    WeightedPoint3::try_new(Point3::try_new(i as Real, y, 0.).unwrap(), w).unwrap()
                })
                .collect(),
            [vec![0.; n], vec![1.; n]].concat(),
        )
        .unwrap()
    }

    #[test]
    fn interior_quadratic_gap_has_a_tight_whole_curve_bound_in_both_directions() {
        for gauge in [1., -2., 1e-280, 1e280] {
            for w in [1., 0.5] {
                let a = curve(&[0., 0., 0.], &[gauge, w * gauge, gauge]);
                let b = curve(&[0., 0.0005, 0.], &[gauge, w * gauge, gauge]);
                let expected = 0.0005 * w / (1. + w);
                for reversed in [false, true] {
                    let b = if reversed {
                        b.reversed().unwrap()
                    } else {
                        b.clone()
                    };
                    let bound = refined_curve_bound(&a, &b, reversed, 1., |_| Ok(()))
                        .unwrap()
                        .unwrap();
                    assert!(bound >= expected);
                    assert!((bound - expected).abs() < 1e-18);
                }
            }
        }
    }

    #[test]
    fn rational_hull_bounds_cover_independent_exact_samples_and_charge_work() {
        for weights in [[1., 0.25, 2., 1.], [1., 4., 0.5, 1.]] {
            let heights = [0.001, -0.002, 0.0007, 0.];
            let a = curve(&[0.; 4], &weights);
            let b = curve(&heights, &weights);
            let mut work = 0;
            let bound = refined_curve_bound(&a, &b, false, 1., |n| {
                work += n;
                Ok(())
            })
            .unwrap()
            .unwrap();
            assert!(work > 4 && work < 100_000);
            assert!(bound <= 0.002);
            for i in 0..=256 {
                let t = rational(i as Real / 256.);
                let u = rational(1.) - &t;
                let basis = [
                    &u * &u * &u,
                    rational(3.) * &u * &u * &t,
                    rational(3.) * &u * &t * &t,
                    &t * &t * &t,
                ];
                let denominator: Rational = basis
                    .iter()
                    .zip(weights)
                    .map(|(b, w)| b * rational(w))
                    .sum();
                let numerator: Rational = basis
                    .iter()
                    .zip(weights)
                    .zip(heights)
                    .map(|((b, w), y)| b * rational(w) * rational(y))
                    .sum();
                let value = numerator / denominator;
                assert!(&value * &value <= rational(bound) * rational(bound));
            }
            assert!(
                refined_curve_bound(&a, &b, false, 1., |_| Err(invalid("test budget"))).is_err()
            );
        }
    }
}
