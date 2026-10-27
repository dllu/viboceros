//! Higher-order correspondence proposals from the existing closest stations.
use crate::{GeometryError, NurbsCurve, NurbsCurve2, Point2, Real};

#[cfg(test)]
pub(super) fn certify(
    source: &NurbsCurve,
    candidate: &NurbsCurve,
    pairs: &[[Real; 2]],
    limit: Real,
) -> Result<bool, GeometryError> {
    Ok(deviation_bound(
        source,
        candidate,
        pairs,
        limit,
        &std::collections::BTreeMap::new(),
    )?
    .is_some())
}

pub(super) fn deviation_bound(
    source: &NurbsCurve,
    candidate: &NurbsCurve,
    pairs: &[[Real; 2]],
    limit: Real,
    derivatives: &std::collections::BTreeMap<u64, [Real; 2]>,
) -> Result<Option<Real>, GeometryError> {
    let Some(map) = proposal(candidate, pairs, derivatives)? else {
        return Ok(None);
    };
    match source.monotone_spline_curve_deviation_bound(candidate, &map, limit) {
        Ok(bound) => Ok(bound),
        Err(GeometryError::CurveApproximationLimit) => Ok(None),
        Err(error) => Err(error),
    }
}

fn proposal(
    candidate: &NurbsCurve,
    pairs: &[[Real; 2]],
    overrides: &std::collections::BTreeMap<u64, [Real; 2]>,
) -> Result<Option<NurbsCurve2>, GeometryError> {
    if pairs.len() < 2
        || pairs.first() != Some(&[0., 0.])
        || pairs.last() != Some(&[1., 1.])
        || pairs.iter().flatten().any(|x| !x.is_finite())
        || pairs.windows(2).any(|p| (0..2).any(|i| p[0][i] >= p[1][i]))
    {
        return Ok(None);
    }
    let widths = pairs
        .windows(2)
        .map(|p| p[1][0] - p[0][0])
        .collect::<Vec<_>>();
    let slopes = pairs
        .windows(2)
        .zip(&widths)
        .map(|(p, h)| (p[1][1] - p[0][1]) / h)
        .collect::<Vec<_>>();
    if slopes.iter().any(|d| !d.is_finite()) {
        return Ok(None);
    }
    let mut derivatives = vec![0.; pairs.len()];
    if slopes.len() == 1 {
        derivatives.fill(slopes[0]);
    } else {
        let endpoint = |h0: Real, h1: Real, m0: Real, m1: Real| {
            ((2. * h0 + h1) / (h0 + h1) * m0 - h0 / (h0 + h1) * m1).max(0.)
        };
        derivatives[0] = endpoint(widths[0], widths[1], slopes[0], slopes[1]);
        for i in 1..pairs.len() - 1 {
            let total = widths[i - 1] + widths[i];
            derivatives[i] = widths[i] / total * slopes[i - 1] + widths[i - 1] / total * slopes[i];
        }
        let n = slopes.len();
        derivatives[n] = endpoint(widths[n - 1], widths[n - 2], slopes[n - 1], slopes[n - 2]);
    }
    if derivatives.iter().any(|d| !d.is_finite()) {
        return Ok(None);
    }
    let point = |fraction| Point2::try_new(candidate.parameter_at(fraction)?, 0.);
    let mut controls = vec![point(0.)?];
    let mut knots = vec![0.; 4];
    for (i, pair) in pairs.windows(2).enumerate() {
        let (a, b) = (pair[0][1], pair[1][1]);
        let delta = b - a;
        let first_derivative = overrides
            .get(&pair[0][0].to_bits())
            .map(|d| d[1])
            .filter(|d| d.is_finite())
            .unwrap_or(derivatives[i]);
        let last_derivative = overrides
            .get(&pair[1][0].to_bits())
            .map(|d| d[0])
            .filter(|d| d.is_finite())
            .unwrap_or(derivatives[i + 1]);
        let mut left = (widths[i] / 3. * first_derivative).clamp(0., delta);
        let mut right = (widths[i] / 3. * last_derivative).clamp(0., delta);
        if left + right > delta {
            let scale = delta / (left + right);
            left *= scale;
            right *= scale;
        }
        let first = (a + left).clamp(a, b);
        let second = (b - right).clamp(first, b);
        controls.extend([point(first)?, point(second)?, point(b)?]);
        knots.extend([pair[1][0]; 3]);
    }
    knots.push(1.);
    Ok(Some(NurbsCurve2::try_new(3, controls, knots)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point3;

    #[test]
    fn cubic_correspondence_certifies_a_nonlinear_clock_with_the_same_stations() {
        let source = NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(1., 0., 0.).unwrap(),
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let line = NurbsCurve::try_new(
            1,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(1., 0., 0.).unwrap(),
            ],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let pairs = [
            [0., 0.],
            [0.25, 0.0625],
            [0.5, 0.25],
            [0.75, 0.5625],
            [1., 1.],
        ];
        assert!(
            source
                .mapped_curve_deviation_bound(&line, &pairs, 1e-12)
                .unwrap()
                .is_none()
        );
        let bound = deviation_bound(
            &source,
            &line,
            &pairs,
            1e-12,
            &std::collections::BTreeMap::new(),
        )
        .unwrap()
        .unwrap();
        assert!(
            bound < 1e-12,
            "retain the proved bound for later composition: {bound}"
        );
    }

    #[test]
    fn matching_stations_do_not_hide_a_feature_from_the_complete_certificate() {
        let source = NurbsCurve::try_new(
            3,
            [[0., 0., 0.], [0., 1., 0.], [1. / 3., -1., 0.], [1., 0., 0.]]
                .into_iter()
                .map(|p| Point3::try_from(p).unwrap())
                .collect(),
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        let line = NurbsCurve::try_new(
            1,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(1., 0., 0.).unwrap(),
            ],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let before = (source.clone(), line.clone());
        assert!(!certify(&source, &line, &[[0., 0.], [0.5, 0.25], [1., 1.]], 1e-6).unwrap());
        assert_eq!((source, line), before);
    }
}
