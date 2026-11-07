//! Source-parameter-independent higher endpoint jets.
use super::*;
impl CurveBlendJet {
    /// Reparameterize a regular source jet so higher endpoint derivatives have
    /// zero tangential component. Odd derivatives reverse with its tangent.
    pub fn from_curve_derivatives(
        position: Point3,
        derivatives: [Vector3; 4],
        reversed: bool,
    ) -> Result<Self, GeometryError> {
        let [d1, d2, d3, d4] = derivatives;
        let speed = d1.length()?;
        let tangent = d1.normalized_nonzero()?;
        let divide = |mut v: Vector3, order: u8| -> Result<Vector3, GeometryError> {
            for _ in 0..order {
                v = Vector3::try_from(v.to_array().map(|x| x / speed))?;
            }
            Ok(v)
        };
        let a2 = divide(d2, 2)?;
        let a3 = divide(d3, 3)?;
        let a4 = divide(d4, 4)?;
        let t = tangent.as_vector();
        let combine = |a: Vector3, b: Vector3, factor: Real| {
            Vector3::try_from(std::array::from_fn(|i| {
                a.to_array()[i] + factor * b.to_array()[i]
            }))
        };
        let normal = |v: Vector3| combine(v, t, -v.dot(t)?);
        let alpha = a2.dot(t)?;
        let beta = a3.dot(t)?;
        let curvature = normal(a2)?;
        let third = normal(combine(a3, a2, -3. * alpha)?)?;
        let fourth = normal(combine(
            combine(a4, a3, -6. * alpha)?,
            a2,
            15. * alpha * alpha - 4. * beta,
        )?)?;
        Ok(Self {
            position,
            tangent: Some(if reversed {
                tangent.opposite()
            } else {
                tangent
            }),
            curvature,
            higher: [
                Some(if reversed { third.scaled(-1.)? } else { third }),
                Some(fourth),
            ],
        })
    }
    /// Fourth derivative target divided by the endpoint speed to the fourth power.
    pub fn with_fourth_derivative(mut self, fourth: Vector3) -> Self {
        self.higher[1] = Some(fourth);
        self
    }

    /// Third/fourth targets divided by the corresponding endpoint speed power.
    pub fn with_higher_derivatives(mut self, third: Vector3, fourth: Vector3) -> Self {
        self.higher = [Some(third), Some(fourth)];
        self
    }
}

#[cfg(test)]
mod higher_tests {
    use super::*;
    #[test]
    fn normalized_fourth_jets_are_invariant_under_nonlinear_source_reparameterization() {
        let point = Point3::try_new(0., 0., 0.).unwrap();
        let vectors = |rows: [[f64; 3]; 4]| rows.map(|v| Vector3::try_from(v).unwrap());
        // C(t)=(t,t²,t³+t⁴), then t=2s+3s²+4s³+5s⁴ at zero.
        let original = vectors([[1., 0., 0.], [0., 2., 0.], [0., 0., 6.], [0., 0., 24.]]);
        let changed = vectors([
            [2., 0., 0.],
            [6., 8., 0.],
            [24., 72., 48.],
            [120., 600., 1248.],
        ]);
        let first = CurveBlendJet::from_curve_derivatives(point, original, false).unwrap();
        let second = CurveBlendJet::from_curve_derivatives(point, changed, false).unwrap();
        assert_eq!(first, second);
        let reverse = CurveBlendJet::from_curve_derivatives(point, changed, true).unwrap();
        assert_eq!(reverse.curvature, first.curvature);
        assert_eq!(
            reverse.higher[0],
            first.higher[0].map(|v| v.scaled(-1.).unwrap())
        );
        assert_eq!(reverse.higher[1], first.higher[1]);
        assert_eq!(reverse.tangent, first.tangent.map(|v| v.opposite()));
    }
}
