//! Representation-dependent fast shortness testing for selection commands.

use crate::{CurveRef, GeometryError, Real, require_finite, vector::product_three};

impl CurveRef<'_> {
    /// Tests shortness using span-wise adaptive three-point quadrature, with
    /// early rejection when a coarse estimate exceeds the remaining budget.
    /// This Rhino-style selection predicate is not a certified length bound:
    /// refinement can change its answer. Use `length` for measured arc length.
    pub fn is_short_for_selection(self, maximum: Real) -> Result<bool, GeometryError> {
        require_finite([maximum], "shortness threshold")?;
        if maximum < 0.0 {
            return Err(GeometryError::InvalidTolerance);
        }
        Shortness {
            remaining: maximum,
            intervals: 0,
        }
        .visit(self)
    }
}

struct Shortness {
    remaining: Real,
    intervals: usize,
}

impl Shortness {
    fn consume(&mut self, length: Real) -> Result<bool, GeometryError> {
        require_finite([length], "shortness length estimate")?;
        if length > self.remaining {
            return Ok(false);
        }
        self.remaining -= length;
        Ok(true)
    }

    fn visit(&mut self, curve: CurveRef<'_>) -> Result<bool, GeometryError> {
        match curve {
            CurveRef::Line(c) => self.consume(c.length()?),
            CurveRef::Circle(c) => self.consume(c.length()?),
            CurveRef::Arc(c) => self.consume(c.length()?),
            CurveRef::Polyline(c) => self.consume(c.length()?),
            CurveRef::Ellipse(c) => self.visit(CurveRef::NurbsCurve(&c.to_nurbs()?)),
            CurveRef::PolyCurve(c) => {
                for segment in c.segments() {
                    if !self.visit(segment.as_ref())? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            CurveRef::NurbsCurve(c) => {
                let c = c.for_arc_length_integration()?;
                let speed = |t| c.derivative_at(t)?.length();
                for (start, end) in c.spans() {
                    let coarse = gauss_three(start, end, &speed)?;
                    if !self.interval(start, end, coarse, 0, &speed)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }

    fn interval(
        &mut self,
        start: Real,
        end: Real,
        coarse: Real,
        depth: usize,
        speed: &impl Fn(Real) -> Result<Real, GeometryError>,
    ) -> Result<bool, GeometryError> {
        if coarse > self.remaining {
            return Ok(false);
        }
        self.intervals += 1;
        if depth >= 24 || self.intervals > 65_536 {
            return Err(GeometryError::NumericalIntegrationDidNotConverge);
        }
        let middle = start * 0.5 + end * 0.5;
        if middle <= start || middle >= end {
            return Err(GeometryError::NumericalIntegrationDidNotConverge);
        }
        let left = gauss_three(start, middle, speed)?;
        let right = gauss_three(middle, end, speed)?;
        let fine = left + right;
        require_finite([fine], "shortness refined estimate")?;
        if (fine - coarse).abs() <= fine * 1e-9 {
            return self.consume(fine);
        }
        if !self.interval(start, middle, left, depth + 1, speed)? {
            return Ok(false);
        }
        self.interval(middle, end, right, depth + 1, speed)
    }
}

fn gauss_three(
    start: Real,
    end: Real,
    speed: &impl Fn(Real) -> Result<Real, GeometryError>,
) -> Result<Real, GeometryError> {
    let center = start * 0.5 + end * 0.5;
    let half_width = end * 0.5 - start * 0.5;
    require_finite([center, half_width], "shortness interval")?;
    if half_width <= 0.0 {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    let offset = half_width * 0.6_f64.sqrt();
    let values = [
        speed(center - offset)?,
        speed(center)?,
        speed(center + offset)?,
    ];
    require_finite(values, "shortness speed")?;
    let scale = values.into_iter().fold(0.0, Real::max);
    if scale == 0.0 {
        return Ok(0.0);
    }
    let sum = (values[0] / scale) * (5.0 / 9.0)
        + (values[1] / scale) * (8.0 / 9.0)
        + (values[2] / scale) * (5.0 / 9.0);
    product_three(half_width, scale, sum, "shortness integral")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Circle3, Point3, Tolerance, Vector3};

    #[test]
    fn shortness_survives_extreme_affine_parameter_domains() {
        let curve = crate::NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(0.5, 1., 0.).unwrap(),
                Point3::try_new(1., 0., 0.).unwrap(),
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        for domain in [
            0.0..=1.0,
            1.0..=f64::from_bits(1.0_f64.to_bits() + 1),
            0.0..=f64::from_bits(1),
            -f64::MAX..=f64::MAX,
            1e100..=f64::from_bits(1e100_f64.to_bits() + 1),
        ] {
            let mapped = curve.try_reparameterized(domain.clone()).unwrap();
            // The arch has exact length sqrt(5)/2 + asinh(2)/4,
            // approximately 1.47894; these thresholds avoid its boundary.
            for (maximum, expected) in [(1., false), (1.4, false), (1.5, true), (2., true)] {
                assert_eq!(
                    CurveRef::NurbsCurve(&mapped).is_short_for_selection(maximum),
                    Ok(expected),
                    "domain {domain:?}, threshold {maximum}"
                );
            }
        }
    }

    #[test]
    fn rational_multispan_shortness_survives_domain_scaling() {
        let circle = Circle3::try_new(
            Point3::try_new(0., 0., 0.).unwrap(),
            0.99999 / std::f64::consts::TAU,
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let curve = circle.to_nurbs().unwrap();
        for domain in [
            0.0..=1e-200,
            0.0..=1e200,
            -f64::MAX..=f64::MAX,
            100.0..=104.0,
        ] {
            let mapped = curve.try_reparameterized(domain.clone()).unwrap();
            assert_eq!(mapped.spans().count(), curve.spans().count());
            // The coarse rational representation deliberately rejects at 1
            // (as in the retained Rhino record), despite true length < 1.
            for (maximum, expected) in [(0.99, false), (1., false), (1.01, true), (2., true)] {
                assert_eq!(
                    CurveRef::NurbsCurve(&mapped).is_short_for_selection(maximum),
                    Ok(expected),
                    "domain {domain:?}, threshold {maximum}"
                );
            }
        }
    }

    #[test]
    fn normalization_must_not_silently_discard_a_span() {
        let curve = crate::NurbsCurve::try_new(
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
        assert_eq!(curve.spans().count(), 3);
        assert!(
            CurveRef::NurbsCurve(&curve)
                .is_short_for_selection(10.)
                .is_err()
        );
    }

    #[test]
    fn quadrature_preserves_constant_speeds_across_extreme_scales() {
        for speed in [0., f64::from_bits(1), 1e-300, 1., 1e300, f64::MAX] {
            let value = gauss_three(0., 1., &|_| Ok(speed)).unwrap();
            if speed == 0. || speed == f64::from_bits(1) {
                assert_eq!(value, speed);
            } else {
                assert!((value / speed - 1.).abs() <= 4. * f64::EPSILON);
            }
        }
        let value = gauss_three(-f64::MAX, f64::MAX, &|_| Ok(f64::from_bits(1))).unwrap();
        assert!(value.is_finite() && value > 0.);
    }

    #[test]
    fn rejection_and_refinement_budget_are_explicit() {
        let mut state = Shortness {
            remaining: 1.,
            intervals: 0,
        };
        assert!(
            !state
                .interval(0., 1., 2., 0, &|_| panic!("rejected before refinement"))
                .unwrap()
        );
        assert_eq!(state.remaining, 1.);
        assert!(matches!(
            state.interval(0., 1., 0.5, 24, &|_| Ok(0.5)),
            Err(GeometryError::NumericalIntegrationDidNotConverge)
        ));
        state.intervals = 65_536;
        assert!(matches!(
            state.interval(0., 1., 0.5, 0, &|_| Ok(0.5)),
            Err(GeometryError::NumericalIntegrationDidNotConverge)
        ));
    }

    #[test]
    fn shortness_does_not_replace_accurate_curve_length() {
        let circle = Circle3::try_new(
            Point3::try_new(0., 0., 0.).unwrap(),
            0.99999 / std::f64::consts::TAU,
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let curve = circle.to_nurbs().unwrap();
        assert!((curve.length(Tolerance::DEFAULT).unwrap() - 0.99999).abs() < 1e-10);
        assert!(
            CurveRef::Circle(&circle)
                .is_short_for_selection(1.)
                .unwrap()
        );
        assert!(
            !CurveRef::NurbsCurve(&curve)
                .is_short_for_selection(1.)
                .unwrap()
        );
        for invalid in [-1., f64::NAN, f64::INFINITY] {
            assert!(
                CurveRef::NurbsCurve(&curve)
                    .is_short_for_selection(invalid)
                    .is_err()
            );
        }
    }
}
