//! Analytic denominator zeros for the quadratic conics used by curve getters.
use crate::{NurbsCurve, Real};
impl NurbsCurve {
    /// Unit-domain denominator roots of a single clamped quadratic whose endpoint
    /// weights are equal and whose middle weight has the opposite sign and at
    /// least their magnitude. Other curves return `None`; this is not a general
    /// certificate that an arbitrary NURBS is pole-free.
    ///
    /// For normalized weights [1,w,1], D(t)=1-2(1-w)t(1-t).
    /// The smaller root is evaluated without subtracting nearly equal numbers.
    /// Roots closer to an endpoint than binary64 can represent round to it.
    pub fn quadratic_pole_fractions(&self) -> Option<[Real; 2]> {
        if self.degree() != 2 || self.control_points().len() != 3 {
            return None;
        }
        let domain = self.domain();
        if !self.knots()[..3].iter().all(|k| *k == *domain.start())
            || !self.knots()[3..].iter().all(|k| *k == *domain.end())
        {
            return None;
        }
        let controls = self.control_points();
        let a = controls[0].weight();
        let b = controls[1].weight();
        let c = controls[2].weight();
        if a != c || a.is_sign_positive() == b.is_sign_positive() || b.abs() < a.abs() {
            return None;
        }
        let ratio = a.abs() / b.abs();
        let discriminant = ((1. - ratio) / (1. + ratio)).sqrt();
        let low = (ratio / (1. + ratio)) / (1. + discriminant);
        Some([low, 1. - low])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GeometryError, Point3, Tolerance, WeightedPoint3};
    fn curve(middle: Real, scale: Real) -> NurbsCurve {
        let points = [
            Point3::try_new(3.75, -3., 0.).unwrap(),
            Point3::try_new(2.4, 0., 0.).unwrap(),
            Point3::try_new(3.75, 3., 0.).unwrap(),
        ];
        NurbsCurve::try_new_rational(
            2,
            points
                .into_iter()
                .zip([scale, middle * scale, scale])
                .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap()
    }
    #[test]
    fn quadratic_poles_reject_infinite_lengths_in_any_common_weight_gauge() {
        for middle in [-1., -2., -5. / 3., -20. / 11.] {
            for scale in [1., -1., 1e200, 1e-200] {
                let curve = curve(middle, scale);
                let [a, b] = curve.quadratic_pole_fractions().unwrap();
                assert!(a > 0. && a <= 0.5 && (0.5..1.).contains(&b));
                assert!((a + b - 1.).abs() < 1e-15);
                for t in [a, b] {
                    let denominator = (1. - t) * (1. - t) + 2. * middle * t * (1. - t) + t * t;
                    assert!(denominator.abs() < 1e-14);
                }
                assert_eq!(
                    curve.length(Tolerance::DEFAULT),
                    Err(GeometryError::ZeroWeightAtParameter)
                );
            }
        }
        let double = curve(-1., 1.);
        assert_eq!(double.quadratic_pole_fractions(), Some([0.5, 0.5]));
        assert!(double.evaluate(0.5).is_err());
        let split = curve(-2., 1.);
        assert!(split.evaluate(0.5).is_ok());
        assert!(split.length(Tolerance::DEFAULT).is_err());
    }
    #[test]
    fn finite_negative_conics_keep_length_and_poles_survive_reparameterization() {
        for w in [-0.999, -0.5, 0.1, 1., 2.] {
            let curve = curve(w, 1.);
            assert_eq!(curve.quadratic_pole_fractions(), None);
            assert!(curve.length(Tolerance::DEFAULT).unwrap() > 0.);
        }
        let source = curve(-2., 1.);
        let unclamped = NurbsCurve::try_new_rational(
            2,
            source.control_points().to_vec(),
            vec![-1., 0., 0., 1., 1., 2.],
        )
        .unwrap();
        assert_eq!(unclamped.quadratic_pole_fractions(), None);
        let mut controls = source.control_points().to_vec();
        controls[2] = WeightedPoint3::try_new(controls[2].point(), 2.).unwrap();
        let unequal = NurbsCurve::try_new_rational(2, controls, source.knots().to_vec()).unwrap();
        assert_eq!(unequal.quadratic_pole_fractions(), None);
        let curve = curve(-2., 1.).try_reparameterized(100.0..=105.0).unwrap();
        assert!(curve.quadratic_pole_fractions().is_some());
        assert!(curve.length(Tolerance::DEFAULT).is_err());
        let extreme = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(Point3::try_new(0., 0., 0.).unwrap(), 1e-300).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(1., 1., 0.).unwrap(), -1e300).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(2., 0., 0.).unwrap(), 1e-300).unwrap(),
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        assert_eq!(extreme.quadratic_pole_fractions(), Some([0., 1.]));
        assert!(extreme.length(Tolerance::DEFAULT).is_err());
    }
}
