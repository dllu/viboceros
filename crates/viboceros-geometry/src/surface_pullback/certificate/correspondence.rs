//! Exact partitions of monotone, normalized piecewise affine parameter maps.
use super::*;

pub(super) fn valid(pairs: &[[Real; 2]]) -> bool {
    (2..=4097).contains(&pairs.len())
        && pairs.first() == Some(&[0., 0.])
        && pairs.last() == Some(&[1., 1.])
        && pairs.iter().flatten().all(|x| x.is_finite())
        && pairs.windows(2).all(|p| (0..2).all(|i| p[0][i] < p[1][i]))
}

/// Each returned interval has source and target bounds, split at every knot
/// crossing of either spline. All calculations retain the exact stored scalars.
pub(super) fn partition(
    pairs: &[[Real; 2]],
    knots: [&[Rational]; 2],
    budget: &mut Budget,
) -> Result<Vec<[[Rational; 2]; 2]>, GeometryError> {
    let mut intervals = Vec::new();
    budget.charge(pairs.len())?;
    for pair in pairs.windows(2) {
        let left = pair[0].map(rational);
        let right = pair[1].map(rational);
        let width = std::array::from_fn::<_, 2, _>(|i| &right[i] - &left[i]);
        let mut cuts = vec![Rational::zero(), Rational::one()];
        for i in 0..2 {
            let first = knots[i].partition_point(|k| k <= &left[i]);
            for k in knots[i][first..].iter().take_while(|k| *k < &right[i]) {
                budget.charge(1)?;
                let t = (k - &left[i]) / &width[i];
                budget.check(&t)?;
                cuts.push(t);
            }
        }
        cuts.sort();
        cuts.dedup();
        for cut in cuts.windows(2) {
            budget.charge(4)?;
            let interval: [[Rational; 2]; 2] =
                std::array::from_fn(|i| std::array::from_fn(|j| &left[i] + &cut[j] * &width[i]));
            for value in interval.iter().flatten() {
                budget.check(value)?;
            }
            intervals.push(interval);
        }
    }
    Ok(intervals)
}
