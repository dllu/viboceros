//! Accuracy-controlled arc-length measurement in a checked integration frame.

use crate::{
    GeometryError, NurbsCurve, Real, Tolerance, integration::integrate_adaptive, require_finite,
};
mod linear;

impl NurbsCurve {
    /// Measures degree-one spans from their straight geometry; higher degrees
    /// use adaptive Gauss-Kronrod integration in a normalized domain.
    pub fn length(&self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        if let Some(spans) = self.rational_line_spans()? {
            let mut sum = crate::FiniteSum::default();
            for span in spans {
                sum.add(span.length)?;
            }
            return sum.total();
        }
        if self.quadratic_pole_fractions().is_some() {
            return Err(GeometryError::ZeroWeightAtParameter);
        }
        let curve = self.for_integration()?;
        let absolute_per_span = tolerance.absolute() / curve.spans().count() as Real;
        if absolute_per_span <= 0.0 {
            return Err(GeometryError::NumericalIntegrationDidNotConverge);
        }
        let mut sum = 0.0;
        let mut correction = 0.0;
        for (start, end) in curve.spans() {
            let length = integrate_adaptive(
                start,
                end,
                absolute_per_span,
                tolerance.relative(),
                |parameter| curve.derivative_at(parameter)?.length(),
            )?;
            let next = sum + length;
            if sum.abs() >= length.abs() {
                correction += (sum - next) + length;
            } else {
                correction += (length - next) + sum;
            }
            sum = next;
        }
        let length = sum + correction;
        require_finite([length], "NURBS curve length")?;
        Ok(length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Circle3, Point3, Vector3};
    use std::borrow::Cow;

    #[test]
    fn periodic_cubic_length_and_signed_endpoints_use_the_original_chart() {
        let points = [
            [0., 0.],
            [1., 3.],
            [3., -1.],
            [5., 0.],
            [0., 0.],
            [1., 3.],
            [3., -1.],
        ];
        let curve = NurbsCurve::try_new(
            3,
            points
                .into_iter()
                .map(|[x, y]| Point3::try_new(x, y, 0.).unwrap())
                .collect(),
            vec![-2., -2., -1., 0., 1., 2., 3., 4., 5., 6., 6.],
        )
        .unwrap();
        let extrapolated = NurbsCurve::try_new(
            3,
            curve.control_points().iter().map(|p| p.point()).collect(),
            vec![-3., -2., -1., 0., 1., 2., 3., 4., 5., 6., 7.],
        )
        .unwrap();
        for curve in [curve, extrapolated] {
            for tolerance in [
                Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap(),
                Tolerance::DEFAULT,
            ] {
                let length = curve.length(tolerance).unwrap();
                assert!((length - 8.81623690546946).abs() < 1e-8);
                let source = crate::Curve3::NurbsCurve(curve.clone());
                for (anchor, signed) in [(0.8, 1.), (0.8, -1.), (3.2, 1.), (3.2, -1.), (0.8, 8.)] {
                    let (piece, end) = source
                        .try_subcurve_at_arc_length_with_endpoint(anchor, signed, tolerance)
                        .unwrap()
                        .unwrap();
                    assert!(
                        (piece.as_ref().length(tolerance).unwrap() - signed.abs()).abs() < 1e-9
                    );
                    assert!(
                        piece
                            .as_ref()
                            .end_point()
                            .unwrap()
                            .distance_to(curve.evaluate(end).unwrap())
                            .unwrap()
                            < 1e-10
                    );
                }
            }
        }
    }

    #[test]
    fn degree_one_rational_length_and_stations_do_not_miss_extreme_weight_motion() {
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let tolerance = Tolerance::try_new(1e-12, 1e-12, 1e-10).unwrap();
        for weights in [
            [1., 2_f64.powi(100)],
            [2_f64.powi(100), 1.],
            [-1., -2_f64.powi(100)],
        ] {
            let curve = NurbsCurve::try_new_rational(
                1,
                vec![
                    crate::WeightedPoint3::try_new(p(0., 0.), weights[0]).unwrap(),
                    crate::WeightedPoint3::try_new(p(2., 0.), weights[1]).unwrap(),
                ],
                vec![0., 0., 1., 1.],
            )
            .unwrap();
            assert!(
                (curve.length(tolerance).unwrap() - 2.).abs() < 1e-12,
                "{weights:?}"
            );
            let samples = crate::CurveRef::NurbsCurve(&curve)
                .divide_by_count_samples(4, true, tolerance)
                .unwrap();
            assert_eq!(samples.len(), 5);
            for (s, x) in samples.iter().zip([0., 0.5, 1., 1.5, 2.]) {
                assert!(
                    (s.point().x() - x).abs() < 1e-12,
                    "{weights:?}: {:?}",
                    s.point()
                );
            }
        }
        let curve = NurbsCurve::try_new_rational(
            1,
            vec![
                crate::WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
                crate::WeightedPoint3::try_new(p(2., 0.), 2_f64.powi(100)).unwrap(),
                crate::WeightedPoint3::try_new(p(2., 3.), 1.).unwrap(),
            ],
            vec![0., 0., 1., 2., 2.],
        )
        .unwrap();
        assert!((curve.length(tolerance).unwrap() - 5.).abs() < 1e-12);
        let samples = crate::CurveRef::NurbsCurve(&curve)
            .divide_by_length_samples(1., true, tolerance)
            .unwrap();
        assert_eq!(samples.len(), 6);
        for (s, expected) in samples.iter().zip([
            p(0., 0.),
            p(1., 0.),
            p(2., 0.),
            p(2., 1.),
            p(2., 2.),
            p(2., 3.),
        ]) {
            assert!(s.point().distance_to(expected).unwrap() < 1e-12);
        }
    }

    #[test]
    fn degree_one_length_rejects_denominator_poles_and_retains_partial_domains() {
        let p = |x| Point3::try_new(x, 0., 0.).unwrap();
        let controls = |w| {
            vec![
                crate::WeightedPoint3::try_new(p(0.), 1.).unwrap(),
                crate::WeightedPoint3::try_new(p(2.), w).unwrap(),
            ]
        };
        let pole = NurbsCurve::try_new_rational(1, controls(-1.), vec![0., 0., 1., 1.]).unwrap();
        assert_eq!(
            pole.length(Tolerance::DEFAULT),
            Err(GeometryError::ZeroWeightAtParameter)
        );
        let curve = NurbsCurve::try_new_rational(1, controls(16.), vec![0., 0., 1., 1.])
            .unwrap()
            .try_trimmed(0.1..=0.8)
            .unwrap();
        let expected = curve
            .evaluate(0.1)
            .unwrap()
            .distance_to(curve.evaluate(0.8).unwrap())
            .unwrap();
        assert!((curve.length(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-12);
    }

    #[test]
    fn non_dyadic_turnaround_length_and_stations_agree_with_exact_total_variation() {
        let p = |x| Point3::try_new(x, 0., 0.).unwrap();
        let curve = NurbsCurve::try_new(
            2,
            vec![p(0.), p(2.5), p(-1.25)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let tolerance = Tolerance::try_new(1e-12, 1e-12, 1e-10).unwrap();
        for domain in [0.0..=1.0, 100.0..=105.0, 0.0..=1e-200, 0.0..=1e200] {
            let mapped = curve.clone().try_reparameterized(domain.clone()).unwrap();
            assert!(
                (mapped.length(tolerance).unwrap() - 3.25).abs() < 1e-11,
                "{domain:?}"
            );
            let stations = crate::CurveRef::NurbsCurve(&mapped)
                .divide_by_count_samples(4, true, tolerance)
                .unwrap();
            assert_eq!(stations.len(), 5);
            for (station, x) in stations.iter().zip([0., 0.8125, 0.375, -0.4375, -1.25]) {
                assert!(
                    (station.point().x() - x).abs() < 1e-10,
                    "{domain:?}: {:?}",
                    station.point()
                );
            }
        }
    }

    #[test]
    fn rational_circle_length_survives_extreme_multispan_domains() {
        let tolerance = Tolerance::try_new(1e-12, 1e-12, 1e-10).unwrap();
        let curve = Circle3::try_new(
            Point3::try_new(0., 0., 0.).unwrap(),
            1.,
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            tolerance,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        for domain in [
            0.0..=1e-200,
            0.0..=1e200,
            -f64::MAX..=f64::MAX,
            100.0..=104.0,
        ] {
            let mapped = curve.try_reparameterized(domain.clone()).unwrap();
            let actual = mapped.length(tolerance).unwrap();
            assert!(
                (actual - std::f64::consts::TAU).abs() < 1e-11,
                "{domain:?}: {actual}"
            );
            let normalized = mapped.for_integration().unwrap();
            assert_eq!(normalized.control_points(), mapped.control_points());
            assert_eq!(normalized.spans().count(), mapped.spans().count());
            assert!(matches!(normalized, Cow::Owned(_)));
            assert!(matches!(
                normalized.for_integration().unwrap(),
                Cow::Borrowed(_)
            ));
        }
    }

    #[test]
    fn degree_one_length_preserves_spans_that_integration_normalization_would_erase() {
        let curve = NurbsCurve::try_new(
            1,
            (0..4)
                .map(|i| Point3::try_new(i as f64, 0., 0.).unwrap())
                .collect(),
            vec![
                -f64::MAX,
                -f64::MAX,
                0.,
                f64::from_bits(1),
                f64::MAX,
                f64::MAX,
            ],
        )
        .unwrap();
        assert!(curve.for_integration().is_err());
        assert_eq!(curve.length(Tolerance::DEFAULT).unwrap(), 3.);
        let sampler = crate::curve::ArcLengthSampler::try_new(
            crate::CurveRef::NurbsCurve(&curve),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(sampler.total_length(), 3.);
        assert_eq!(
            sampler.point_at_distance(1.5).unwrap(),
            Point3::try_new(1.5, 0., 0.).unwrap()
        );
    }

    #[test]
    fn quadratic_length_is_independent_of_extreme_parameter_domains() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(0.5, 1., 0.).unwrap(),
                Point3::try_new(1., 0., 0.).unwrap(),
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let expected = 0.5 * 5_f64.sqrt() + 0.25 * 2_f64.asinh();
        let tolerance = Tolerance::try_new(1e-12, 1e-12, 1e-10).unwrap();
        for domain in [
            0.0..=1.0,
            1.0..=f64::from_bits(1.0_f64.to_bits() + 1),
            0.0..=f64::from_bits(1),
            -f64::MAX..=f64::MAX,
            1e100..=f64::from_bits(1e100_f64.to_bits() + 1),
        ] {
            let mapped = curve.try_reparameterized(domain.clone()).unwrap();
            let actual = mapped.length(tolerance).unwrap();
            assert!((actual - expected).abs() < 1e-12, "{domain:?}: {actual}");
        }
    }
}
