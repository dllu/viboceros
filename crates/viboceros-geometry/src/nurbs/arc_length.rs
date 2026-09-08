//! Arc-length measurement and dimensionless integration preparation.

use std::borrow::Cow;

use crate::{
    GeometryError, NurbsCurve, Real, Tolerance, integration::integrate_adaptive, require_finite,
};

impl NurbsCurve {
    /// Borrows unit-domain curves, otherwise maps their full knot vector to
    /// a dimensionless frame without changing controls or weights. This
    /// avoids parameter-scale derivative overflow and sampling outside narrow
    /// translated domains. It does not fix arbitrarily ill-conditioned
    /// relative interior span widths.
    pub(crate) fn for_arc_length_integration(&self) -> Result<Cow<'_, Self>, GeometryError> {
        if self.domain() == (0.0..=1.0) {
            return Ok(Cow::Borrowed(self));
        }
        let normalized = self.try_reparameterized(0.0..=1.0)?;
        // Never silently remove an interval whose relative width cannot be
        // represented in the normalized frame, including exterior knots.
        if self
            .knots()
            .windows(2)
            .zip(normalized.knots().windows(2))
            .any(|(before, after)| before[0] < before[1] && after[0] >= after[1])
        {
            return Err(GeometryError::NumericalIntegrationDidNotConverge);
        }
        Ok(Cow::Owned(normalized))
    }

    /// Computes arc length span by span with adaptive Gauss-Kronrod
    /// integration of the exact first derivative in a normalized domain.
    pub fn length(&self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        let curve = self.for_arc_length_integration()?;
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
            let normalized = mapped.for_arc_length_integration().unwrap();
            assert_eq!(normalized.control_points(), mapped.control_points());
            assert_eq!(normalized.spans().count(), mapped.spans().count());
            assert!(matches!(normalized, Cow::Owned(_)));
            assert!(matches!(
                normalized.for_arc_length_integration().unwrap(),
                Cow::Borrowed(_)
            ));
        }
    }

    #[test]
    fn length_rejects_normalization_that_erases_an_interval() {
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
        assert!(curve.length(Tolerance::DEFAULT).is_err());
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
