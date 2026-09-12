//! Whole-span circular-locus recognition, without sparse point sampling.
use super::*;

impl NurbsCurve {
    /// Returns a candidate circle radius only when every rational Bezier span
    /// lies within absolute tolerance of its plane and sphere.
    /// Same-sign span weights are required for a denominator lower bound.
    /// Inconclusive or ill-conditioned coefficient bounds return `None`.
    pub fn circular_radius(&self, tolerance: Tolerance) -> Result<Option<Real>, GeometryError> {
        if self.degree() < 2 {
            return Ok(None);
        }
        let parameter = *self.domain().start();
        let start = self.evaluate(parameter)?;
        let curvature = CurveRef::NurbsCurve(self).curvature_vector(parameter)?;
        let magnitude = curvature.length()?;
        if magnitude == 0. {
            return Ok(None);
        }
        let radius = 1. / magnitude;
        if !radius.is_finite() || radius <= tolerance.absolute() {
            return Ok(None);
        }
        let inward = curvature.normalized_nonzero()?.as_vector();
        let center = start.translated(inward.scaled(radius)?)?;
        let tangent = self
            .derivative_at(parameter)?
            .normalized_nonzero()?
            .as_vector();
        let normal = inward.cross(tangent)?.normalized_nonzero()?.as_vector();
        let delta = (tolerance.absolute() / radius).min(0.25);
        for span in self.try_bezier_spans()? {
            if !span_on_circle(&span, center, radius, normal, delta)? {
                return Ok(None);
            }
        }
        Ok(Some(radius))
    }
}

fn span_on_circle(
    span: &NurbsCurve,
    center: Point3,
    radius: Real,
    normal: Vector3,
    delta: Real,
) -> Result<bool, GeometryError> {
    let controls = span.control_points();
    let gauge = controls
        .iter()
        .map(|p| p.weight().abs())
        .fold(0., Real::max);
    let sign = controls[0].weight().signum();
    let mut q = Vec::with_capacity(controls.len());
    let mut minimum_weight = 1.0_f64;
    for control in controls {
        let w = control.weight() / gauge * sign;
        if w <= 0. {
            return Ok(false);
        }
        minimum_weight = minimum_weight.min(w);
        let offset = center.vector_to(control.point())?.scaled(1. / radius)?;
        // Positive rational weights keep plane distance within the control hull.
        if offset.dot(normal)?.abs() + 32. * Real::EPSILON * offset.length()? > delta {
            return Ok(false);
        }
        q.push((offset.scaled(w)?, w));
    }
    let n = span.degree();
    let binomial = |degree: usize| {
        let mut values = vec![1.; degree + 1];
        for i in 1..=degree {
            values[i] = values[i - 1] * (degree + 1 - i) as Real / i as Real;
        }
        values
    };
    let b = binomial(n);
    let doubled = binomial(2 * n);
    let bound = delta * (2. - delta) * minimum_weight * minimum_weight;
    if bound == 0. {
        return Ok(false);
    }
    // Bernstein coefficients of Q.Q - W.W bound the whole rational locus.
    // Dividing by min(W)^2 converts the residual to squared radial distance.
    for (k, denominator) in doubled.into_iter().enumerate() {
        let mut residual = 0.;
        let mut absolute_products = 0.;
        for i in k.saturating_sub(n)..=k.min(n) {
            let j = k - i;
            let factor = b[i] * b[j] / denominator;
            let dot = q[i].0.dot(q[j].0)?;
            let weight = q[i].1 * q[j].1;
            residual += factor * (dot - weight);
            absolute_products += factor * (q[i].0.length()? * q[j].0.length()? + weight);
        }
        let roundoff = 64. * (n + 1) as Real * Real::EPSILON * absolute_products;
        if !residual.is_finite() || !roundoff.is_finite() || residual.abs() + roundoff > bound {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circularity_rejects_a_bump_invisible_to_three_sample_second_order_jets() {
        let circle = crate::Circle3::try_new(
            Point3::try_new(0., 0., 0.).unwrap(),
            2.,
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let arc = circle
            .try_bezier_spans()
            .unwrap()
            .remove(0)
            .try_reparameterized(0.0..=1.0)
            .unwrap()
            .try_change_degree(9, false)
            .unwrap();
        // Add t^3(t-1)^3(t-1/2)^3 to the homogeneous x numerator.
        // Position, first and second derivatives at 0, 1/2 and 1 are unchanged.
        let mut power = vec![1.];
        for root in [0., 0., 0., 1., 1., 1., 0.5, 0.5, 0.5] {
            let mut next = vec![0.; power.len() + 1];
            for (i, coefficient) in power.iter().enumerate() {
                next[i] -= root * coefficient;
                next[i + 1] += coefficient;
            }
            power = next;
        }
        let choose =
            |n: usize, k: usize| (0..k).fold(1., |v, j| v * (n - j) as f64 / (j + 1) as f64);
        let controls = arc
            .control_points()
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let bump: f64 = (0..=i)
                    .map(|j| power[j] * choose(i, j) / choose(9, j))
                    .sum();
                WeightedPoint3::try_new(
                    p.point()
                        .translated(Vector3::try_new(100. * bump / p.weight(), 0., 0.).unwrap())
                        .unwrap(),
                    p.weight(),
                )
                .unwrap()
            })
            .collect();
        let perturbed = NurbsCurve::try_new_rational(9, controls, arc.knots().to_vec()).unwrap();
        for t in [0., 0.5, 1.] {
            let (a, da, dda) = arc.evaluate_with_second_derivative(t).unwrap();
            let (b, db, ddb) = perturbed.evaluate_with_second_derivative(t).unwrap();
            assert!(a.distance_to(b).unwrap() < 1e-10);
            for (x, y) in da
                .to_array()
                .into_iter()
                .zip(db.to_array())
                .chain(dda.to_array().into_iter().zip(ddb.to_array()))
            {
                assert!((x - y).abs() < 1e-9);
            }
        }
        assert!(arc.circular_radius(Tolerance::DEFAULT).unwrap().is_some());
        assert!(
            perturbed
                .circular_radius(Tolerance::DEFAULT)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn circularity_checks_rational_arcs_degree_elevation_and_noncircles() {
        let point = |x, y| Point3::try_new(x, y, 0.).unwrap();
        for radius_y in [2., 4.] {
            let curve = NurbsCurve::try_new_rational(
                2,
                vec![
                    WeightedPoint3::try_new(point(2., 0.), 1.).unwrap(),
                    WeightedPoint3::try_new(point(2., radius_y), std::f64::consts::FRAC_1_SQRT_2)
                        .unwrap(),
                    WeightedPoint3::try_new(point(0., radius_y), 1.).unwrap(),
                ],
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap();
            for degree in [2, 5, 12] {
                let elevated = curve.try_change_degree(degree, false).unwrap();
                let radius = elevated.circular_radius(Tolerance::DEFAULT).unwrap();
                if radius_y == 2. {
                    assert!((radius.unwrap() - 2.).abs() < 1e-10);
                } else {
                    assert!(radius.is_none());
                }
            }
        }
    }
}
