//! Shared numerical batch tolerance and constant-speed NURBS regressions.
use super::*;
pub(super) fn batch_tolerance(fractional_tolerance: Real) -> Real {
    // Cold-curve resolution captures expose the transition at 1e-5 and the
    // coarse-request cap. Independent spatial holdouts exercise both sides
    // of these thresholds, natural knots and interior quadrature checkpoints.
    if fractional_tolerance < 1e-5 {
        fractional_tolerance
    } else {
        fractional_tolerance.min(0.01) * 0.01
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(scale: Real, reverse: bool, domain: [Real; 3]) -> crate::NurbsCurve {
        let mut points = [[0., 0., 0.], [1., 0., 0.], [2., 1e-5, 0.]];
        if reverse {
            points.reverse();
        }
        crate::NurbsCurve::try_new_rational(
            1,
            points
                .into_iter()
                .map(|point| {
                    crate::WeightedPoint3::try_new(
                        Point3::try_from(point.map(|v| v * scale)).unwrap(),
                        1.,
                    )
                    .unwrap()
                })
                .collect(),
            vec![domain[0], domain[0], domain[1], domain[2], domain[2]],
        )
        .unwrap()
    }

    #[test]
    fn numerical_boundaries_preserve_scaled_sources_and_precise_queries() {
        for scale in [1e-200, 1., 1e200] {
            for reverse in [false, true] {
                let curve = source(scale, reverse, [-7., 9., 25.]);
                let before = curve.clone();
                let sampler =
                    ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT)
                        .unwrap();
                let fractions = [0., 0.125, 0.5, 0.875, 1.];
                let precise = sampler
                    .parameters_at_normalized_lengths(&fractions)
                    .unwrap();
                let numerical = sampler
                    .parameters_at_incremental_normalized_lengths(&fractions, 1e-4)
                    .unwrap();
                assert_eq!([numerical[0], numerical[2], numerical[4]], [-7., 9., 25.]);
                for index in [1, 3] {
                    assert!((numerical[index] - precise[index]).abs() < 1e-12);
                }
                assert_ne!(precise[2], numerical[2]);
                assert_eq!(
                    precise,
                    sampler
                        .parameters_at_normalized_lengths(&fractions)
                        .unwrap()
                );
                assert_eq!(curve, before);
            }
        }
    }

    #[test]
    fn nearby_fractions_can_keep_a_previous_non_knot_root() {
        let curve = source(1., false, [0., 1., 2.]);
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        let center = 0.06 / sampler.total_length();
        let fractions = [
            0.,
            center - 1.1e-6,
            center - 0.9e-6,
            center,
            center + 0.9e-6,
            center + 1.1e-6,
            1.,
        ];
        let values = sampler
            .parameters_at_incremental_normalized_lengths(&fractions, 1e-6)
            .unwrap();
        assert_eq!(values[1], values[2]);
        assert_ne!(values[1], 0.06);
        assert!((values[3] - 0.06).abs() < 1e-15);
        assert_eq!(values[3], values[4]);
        assert!(values[5] > values[4]);
        assert_eq!([values[0], values[6]], [0., 2.]);
    }
}
