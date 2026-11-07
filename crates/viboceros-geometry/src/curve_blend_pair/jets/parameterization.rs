//! Scalar parameter changes retain geometric endpoint jets and add shape freedoms.
use super::*;
use crate::exact_scalar::{rational, scalar};

impl CurveBlendJet {
    /// Compose with a scalar parameter change whose first derivative is one and
    /// whose next derivatives are supplied here. Endpoint speed subsequently
    /// scales derivative order k by speed^k.
    pub fn with_parameterization_derivatives(
        mut self,
        derivatives: [Real; 3],
    ) -> Result<Self, GeometryError> {
        crate::require_finite(derivatives, "curve blend scalar parameter derivatives")?;
        self.parameterization = derivatives;
        self.spacing = None;
        Ok(self)
    }
    /// Tangential gaps of the second, third and fourth Bézier controls, relative
    /// to the first handle. Ratios may be negative; one preserves the default.
    /// G2/G3 use scalar composition. G4 uses the independently measured BlendCrv
    /// control response, K*(12a*(b+K.K)+9a³), in place of K*(4b+3a²).
    /// This command policy does not assert mathematical fourth-order continuity;
    /// use `with_parameterization_derivatives` for ordinary scalar composition.
    pub fn with_control_spacing_ratios(mut self, ratios: [Real; 3]) -> Result<Self, GeometryError> {
        crate::require_finite(ratios, "curve blend control spacing ratios")?;
        self.spacing = (ratios != [1.; 3]).then_some(ratios);
        self.parameterization = [0.; 3];
        Ok(self)
    }
    pub(super) fn parameterized_derivatives(
        self,
        handle: Real,
        degree: usize,
        order: u8,
        end: bool,
    ) -> Result<[[Real; 3]; 4], GeometryError> {
        let direction = self.tangent.expect("constrained blend tangent").as_vector();
        let speed = degree as Real * handle;
        crate::require_finite([speed], "curve blend endpoint speed")?;
        let mut out = [[0.; 3]; 4];
        // Preserve the established operation order and endpoint bits for defaults.
        out[0] = direction.scaled(handle)?.scaled(degree as Real)?.to_array();
        let zero = Vector3::try_new(0., 0., 0.)?;
        let mut base = [direction, self.curvature, zero, zero];
        for k in 3..=order {
            base[usize::from(k - 1)] = self.higher[usize::from(k - 3)].ok_or(
                GeometryError::InvalidEndpointInterpolation {
                    context: "missing higher curve endpoint derivative",
                },
            )?;
        }
        let mut parameters = self.parameterization;
        if let Some(ratios) = self.spacing {
            for k in 2..=order {
                let Some(raw) = super::spacing::tangent_derivative(ratios, handle, degree, k, end)
                else {
                    return super::spacing::exact_derivatives(
                        base, ratios, handle, degree, order, end, out,
                    );
                };
                let mut value = raw;
                for _ in 0..k {
                    value /= speed;
                }
                if !raw.is_finite() || !value.is_finite() || (raw != 0. && !value.is_normal()) {
                    return super::spacing::exact_derivatives(
                        base, ratios, handle, degree, order, end, out,
                    );
                }
                parameters[usize::from(k - 2)] = value;
            }
        }
        if parameters == [0.; 3] {
            for k in 2..=order {
                let mut value = base[usize::from(k - 1)].to_array();
                for _ in 0..k {
                    value = value.map(|v| v * speed);
                }
                out[usize::from(k - 1)] = value;
            }
            return Ok(out);
        }
        let [a, b, c] = parameters;
        for (axis, [t, k, j, l]) in (0..3)
            .map(|axis| base.map(|v| v.to_array()[axis]))
            .enumerate()
        {
            let fourth_curvature_terms = if self.spacing.is_some() {
                let mut terms = vec![vec![12., a, b, k], vec![9., a, a, a, k]];
                terms.extend(base[1].to_array().map(|v| vec![12., a, v, v, k]));
                terms
            } else {
                vec![vec![4., b, k], vec![3., a, a, k]]
            };
            let mut fourth = vec![vec![l], vec![6., a, j], vec![c, t]];
            fourth.extend(fourth_curvature_terms);
            let terms = [
                vec![vec![k], vec![a, t]],
                vec![vec![j], vec![3., a, k], vec![b, t]],
                fourth,
            ];
            for (n, terms) in terms
                .into_iter()
                .enumerate()
                .take(usize::from(order.saturating_sub(1)))
            {
                out[n + 1][axis] = sum_products(terms, speed, n + 2)?;
            }
        }
        Ok(out)
    }
}
fn sum_products(
    mut terms: Vec<Vec<Real>>,
    speed: Real,
    order: usize,
) -> Result<Real, GeometryError> {
    terms.retain(|term| !term.contains(&0.));
    if terms.is_empty() {
        return Ok(0.);
    }
    let mut sum = 0.;
    let mut magnitude = 0.;
    let mut recover = false;
    for term in &mut terms {
        term.extend(std::iter::repeat_n(speed, order));
        let mut product = 1.;
        for factor in term.iter() {
            product *= factor;
            recover |= !product.is_normal();
        }
        sum += product;
        magnitude += product.abs();
        recover |= !sum.is_finite();
    }
    recover |= sum.abs() <= 64. * Real::EPSILON * magnitude;
    if !recover {
        return Ok(sum);
    }
    scalar(
        &terms
            .iter()
            .map(|term| {
                term.iter()
                    .map(|v| rational(*v))
                    .product::<crate::exact_scalar::Rational>()
            })
            .sum(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn v(a: [Real; 3]) -> Vector3 {
        Vector3::try_from(a).unwrap()
    }
    #[test]
    fn scalar_composition_matches_an_independent_polynomial_chain_through_fourth_order() {
        let jet = CurveBlendJet::new(
            Point3::try_new(0., 0., 0.).unwrap(),
            Some(v([1., 0., 0.]).normalized_nonzero().unwrap()),
            v([0., 2., 0.]),
        )
        .with_higher_derivatives(v([0., 0., 6.]), v([0., 0., 24.]))
        .with_parameterization_derivatives([1.5, 3., 7.5])
        .unwrap();
        // C(t)=(t,t²,t³+t⁴), t=2u+3u²+4u³+5u⁴.
        let expected = [
            [2., 0., 0.],
            [6., 8., 0.],
            [24., 72., 48.],
            [120., 600., 1248.],
        ];
        let actual = jet.parameterized_derivatives(2. / 9., 9, 4, false).unwrap();
        for (a, b) in actual
            .into_iter()
            .flatten()
            .zip(expected.into_iter().flatten())
        {
            assert!((a - b).abs() < 1e-10);
        }
        assert!(
            jet.with_parameterization_derivatives([Real::NAN, 0., 0.])
                .is_err()
        );
    }
    #[test]
    fn product_sum_recovers_underflow_overflow_and_cancellation_before_rounding() {
        assert_eq!(
            sum_products(vec![vec![1e-300, 1e-300]], 1e150, 4).unwrap(),
            scalar(&(rational(1e-300) * rational(1e-300) * rational(1e150).pow(4))).unwrap()
        );
        assert_eq!(
            sum_products(
                vec![vec![1e300, 1e300], vec![-1e300, 1e300], vec![2.]],
                1.,
                2
            )
            .unwrap(),
            2.
        );
    }
    #[test]
    fn overflowing_endpoint_speed_returns_an_error_before_exact_conversion() {
        let zero = v([0.; 3]);
        let jet = CurveBlendJet::new(
            Point3::try_new(0., 0., 0.).unwrap(),
            Some(v([1.; 3]).normalized_nonzero().unwrap()),
            zero,
        )
        .with_higher_derivatives(zero, zero)
        .with_control_spacing_ratios([2., 1., 1.])
        .unwrap();
        // Each component of D1 can be finite although its speed overflows.
        // Exact conversion must receive finite factors, never panic on infinity.
        assert!(
            jet.parameterized_derivatives(Real::MAX / 2., 3, 2, false)
                .is_err()
        );
    }
}
