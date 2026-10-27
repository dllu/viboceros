//! Reuse known fitting-span correspondences without additional closest queries.
use super::*;

type AnchoredStations = (Stations, BTreeMap<u64, [Real; 2]>);

pub(super) fn prepare(
    source: &NurbsCurve,
    candidate: &NurbsCurve,
    pairs: &[[Real; 2]],
) -> Result<Option<AnchoredStations>, GeometryError> {
    if pairs.len() < 2
        || pairs.len() > MAX_INTERVALS + 1
        || pairs.first() != Some(&[0., 0.])
        || pairs.last() != Some(&[1., 1.])
        || pairs.iter().flatten().any(|x| !x.is_finite())
        || pairs.windows(2).any(|p| (0..2).any(|i| p[0][i] >= p[1][i]))
    {
        return Ok(None);
    }
    let domain = candidate.domain();
    let knots = candidate.knots()[candidate.degree()..=candidate.control_points().len()]
        .chunk_by(|a, b| a == b)
        .map(|g| crate::nurbs::interval_fraction(g[0], *domain.start(), *domain.end()))
        .collect::<Result<Vec<_>, _>>()?;
    if knots.len() != pairs.len() || knots.iter().zip(pairs).any(|(k, p)| *k != p[1]) {
        return Ok(None);
    }
    let mut stations = Stations::new(source)?;
    for pair in pairs {
        stations.parameters.insert(pair[0].to_bits(), Some(pair[1]));
    }
    if stations.parameters.len() > MAX_INTERVALS + 1 {
        return Ok(None);
    }
    stations.gaps.clear();
    let keys = stations.parameters.keys().copied().collect::<Vec<_>>();
    for pair in keys.windows(2) {
        let width = Real::from_bits(pair[1]) - Real::from_bits(pair[0]);
        stations.gaps.push((width.to_bits(), pair[0], pair[1]));
    }
    let source_sampler = source.parameter_sampler()?;
    let target_sampler = candidate.parameter_sampler()?;
    let mut derivatives = BTreeMap::new();
    for (pair, span) in pairs.windows(2).zip(target_sampler.spans()) {
        let width = pair[1][1] - pair[0][1];
        for (point, side, fraction) in [(pair[0], 1, 0.), (pair[1], 0, 1.)] {
            let a = source_sampler.evaluate_with_derivative(point[0])?.1;
            let b = span.evaluate_with_derivative(fraction)?.1;
            if let Ok(direction) = b.normalized_nonzero() {
                let derivative = width * a.dot(direction.as_vector())? / b.length()?;
                if derivative.is_finite() {
                    derivatives
                        .entry(point[0].to_bits())
                        .or_insert([Real::NAN; 2])[side] = derivative.max(0.);
                }
            }
        }
    }
    Ok(Some((stations, derivatives)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point3;

    #[test]
    fn known_span_boundaries_keep_one_sided_parameter_speeds() {
        let point = |x| Point3::try_new(x, 0., 0.).unwrap();
        let source = NurbsCurve::try_new(
            2,
            vec![point(0.), point(0.), point(1.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let candidate = NurbsCurve::try_new(
            1,
            vec![point(0.), point(0.25), point(1.)],
            vec![0., 0., 0.5, 1., 1.],
        )
        .unwrap();
        let pairs = [[0., 0.], [0.5, 0.5], [1., 1.]];
        assert!(!super::super::hermite::certify(&source, &candidate, &pairs, 1e-12).unwrap());
        for chart in [0. ..=1., -3. ..=17., 100. ..=107.] {
            let target = candidate.try_reparameterized(chart).unwrap();
            let before = (source.clone(), target.clone());
            assert!(
                super::super::within_deviation_at_pairs(&source, &target, 1e-12, &pairs).unwrap()
            );
            assert_eq!((source.clone(), target), before);
        }
    }
}
