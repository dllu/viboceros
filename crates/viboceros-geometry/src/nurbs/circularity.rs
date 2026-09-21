//! Whole-span circular-locus recognition with a center-stability screen.
use super::*;

impl NurbsCurve {
    /// Returns a candidate circle radius only when every rational Bezier span
    /// lies within absolute tolerance of its plane and sphere.
    /// Same-sign span weights are required for a denominator lower bound.
    /// Regular curvature jets must also agree on center and radius, so a short
    /// ellipse inside a circle's tolerance tube is not sufficient evidence.
    /// Inconclusive or ill-conditioned coefficient bounds return `None`.
    pub fn circular_radius(&self, tolerance: Tolerance) -> Result<Option<Real>, GeometryError> {
        Ok(self.circular_locus(tolerance)?.map(|(_, radius)| radius))
    }

    /// Recognizes the center of a circular locus using all rational Bezier
    /// spans, not a fit to a few sampled points. Partial arcs are eligible.
    /// This does not replace the curve or assert its sweep/parameterization.
    /// Same-sign span weights and absolute plane/radial bounds are required;
    /// inconclusive or ill-conditioned recognition returns `None`.
    pub fn circular_center(&self, tolerance: Tolerance) -> Result<Option<Point3>, GeometryError> {
        Ok(self.circular_locus(tolerance)?.map(|(center, _)| center))
    }

    fn circular_locus(
        &self,
        tolerance: Tolerance,
    ) -> Result<Option<(Point3, Real)>, GeometryError> {
        if self.degree() < 2 {
            return Ok(None);
        }
        let spans = self.try_bezier_spans()?;
        let mut proposal: Option<(Point3, Real, Vector3)> = None;
        for span in &spans {
            // A scalar parameter domain must not overflow geometric curvature
            // through its first/second derivatives. Normalize only this seed;
            // the whole-span test below retains the original control geometry.
            let origin = span.control_points()[0].point();
            let controls = span
                .control_points()
                .iter()
                .map(|p| {
                    WeightedPoint3::try_new(
                        Point3::try_from(origin.vector_to(p.point())?.to_array())?,
                        p.weight(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let seed =
                NurbsCurve::try_new_rational(span.degree(), controls, span.knots().to_vec())?
                    .try_reparameterized(0. ..=1.)?;
            for t in [0., 0.5, 1.] {
                let Some((local_center, radius, normal)) =
                    circle_proposal(&seed, t, tolerance).ok().flatten()
                else {
                    continue;
                };
                let center = origin.translated(Vector3::try_from(local_center.to_array())?)?;
                if let Some((previous, previous_radius, _)) = proposal {
                    // A short ellipse can lie in a circle's tolerance tube even
                    // though its osculating center changes appreciably. Stable
                    // jets are necessary, but not sufficient: whole-span bounds
                    // below still reject bumps invisible to these samples.
                    if previous.distance_to(center)? > tolerance.absolute()
                        || (previous_radius - radius).abs() > tolerance.absolute()
                    {
                        return Ok(None);
                    }
                } else {
                    proposal = Some((center, radius, normal));
                }
            }
        }
        let Some((center, radius, normal)) = proposal else {
            return Ok(None);
        };
        if self.degree() == 2
            && !super::ellipticity::quadratic_circle_consistent(&spans, center, radius, tolerance)?
        {
            return Ok(None);
        }
        let delta = (tolerance.absolute() / radius).min(0.25);
        for span in &spans {
            if !span_on_circle(span, center, radius, normal, delta)? {
                return Ok(None);
            }
        }
        Ok(Some((center, radius)))
    }
}

fn circle_proposal(
    seed: &NurbsCurve,
    parameter: Real,
    tolerance: Tolerance,
) -> Result<Option<(Point3, Real, Vector3)>, GeometryError> {
    let start = seed.evaluate(parameter)?;
    let curvature = CurveRef::NurbsCurve(seed).curvature_vector(parameter)?;
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
    let tangent = seed
        .derivative_at(parameter)?
        .normalized_nonzero()?
        .as_vector();
    let normal = inward.cross(tangent)?.normalized_nonzero()?.as_vector();
    Ok(Some((center, radius, normal)))
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
    unit_sphere_bound(&q, minimum_weight, delta)
}

pub(super) fn binomial(degree: usize) -> Vec<Real> {
    let mut values = vec![1.; degree + 1];
    for i in 1..=degree {
        values[i] = values[i - 1] * (degree + 1 - i) as Real / i as Real;
    }
    values
}

/// A positive rational denominator and Bernstein numerator residual bound the
/// entire unit-sphere locus. Ellipses reuse this after an affine planar map.
pub(super) fn unit_sphere_bound(
    q: &[(Vector3, Real)],
    minimum_weight: Real,
    delta: Real,
) -> Result<bool, GeometryError> {
    let n = q.len() - 1;
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
    fn radial_tube_alone_does_not_make_a_short_ellipse_circular() {
        let origin = Point3::try_new(0., 0., 0.).unwrap();
        let x = Vector3::try_new(1., 0., 0.)
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        let y = Vector3::try_new(0., 1., 0.)
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        let ellipse = crate::Ellipse3::try_new(origin, 2., 1., x, y, Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap();
        for end in [1e-3, 1e-6] {
            let arc = ellipse.try_trimmed(0. ..=end).unwrap();
            let seed = arc.try_reparameterized(0. ..=1.).unwrap();
            let (center, radius, normal) = circle_proposal(&seed, 0., Tolerance::DEFAULT)
                .unwrap()
                .unwrap();
            assert!(center.distance_to(origin).unwrap() > 1.);
            assert!(
                span_on_circle(
                    &seed,
                    center,
                    radius,
                    normal,
                    Tolerance::DEFAULT.absolute() / radius
                )
                .unwrap()
            );
            assert!(arc.circular_center(Tolerance::DEFAULT).unwrap().is_none());
            assert!(arc.circular_radius(Tolerance::DEFAULT).unwrap().is_none());
        }
    }

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
        assert!(
            perturbed
                .circular_center(Tolerance::DEFAULT)
                .unwrap()
                .is_none()
        );
        assert!(
            perturbed
                .elliptical_center(Tolerance::DEFAULT)
                .unwrap()
                .is_none()
        );
        assert!(
            perturbed
                .try_canonical_circular_arc(Tolerance::DEFAULT)
                .unwrap()
                .is_none()
        );
        let extended = perturbed
            .try_merged_naturally_by_length(CurveExtensionSide::End, 0.1, Tolerance::DEFAULT)
            .unwrap();
        for i in 0..=32 {
            let t = i as Real / 32.;
            assert!(
                perturbed
                    .evaluate(t)
                    .unwrap()
                    .distance_to(extended.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-9
            );
        }
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

    #[test]
    fn circular_centers_ignore_native_domains_weight_gauges_refinement_and_direction() {
        let normal = Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        for origin in [0., 1e12] {
            let center = Point3::try_new(origin + 4., origin - 4., 3.).unwrap();
            let circle = crate::Circle3::try_new(center, 2., normal, Tolerance::DEFAULT)
                .unwrap()
                .to_nurbs()
                .unwrap();
            let arc = circle.try_bezier_spans().unwrap().remove(0);
            for source in [circle, arc] {
                for domain in [0. ..=1., 1e12..=1e12 + 8., 0. ..=1e-170, 0. ..=1e170] {
                    for gauge in [1., -8., 1e-200, -1e200] {
                        let scaled = NurbsCurve::try_new_rational(
                            source.degree(),
                            source
                                .control_points()
                                .iter()
                                .map(|p| {
                                    WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap()
                                })
                                .collect(),
                            source.knots().to_vec(),
                        )
                        .unwrap()
                        .try_reparameterized(domain.clone())
                        .unwrap();
                        for curve in [scaled.clone(), scaled.reversed().unwrap()] {
                            let before = curve.clone();
                            let actual =
                                curve.circular_center(Tolerance::DEFAULT).unwrap().unwrap();
                            assert!(
                                actual.distance_to(center).unwrap() < 1e-9,
                                "{origin} {domain:?} {gauge}: {actual:?}"
                            );
                            assert!(
                                (curve.circular_radius(Tolerance::DEFAULT).unwrap().unwrap() - 2.)
                                    .abs()
                                    < 1e-9
                            );
                            assert_eq!(curve, before);
                        }
                    }
                }
            }
        }
        let center = Point3::try_new(4., -4., 3.).unwrap();
        let curve = crate::Circle3::try_new(center, 2., normal, Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap();
        for degree in [2, 5, 12] {
            let elevated = curve.try_change_degree(degree, false).unwrap();
            let knot = elevated.parameter_at(0.125).unwrap();
            let refined = elevated.try_insert_knot(knot, 1).unwrap();
            assert!(
                refined
                    .circular_center(Tolerance::DEFAULT)
                    .unwrap()
                    .unwrap()
                    .distance_to(center)
                    .unwrap()
                    < 1e-9
            );
        }
    }

    #[test]
    fn circular_center_can_propose_from_a_regular_point_after_a_stationary_endpoint() {
        // A quadratic rational quarter circle composed with t -> t^2.
        // Its homogeneous quartic controls are H0,H0,(2H0+H1)/3,H1,H2.
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let curve = NurbsCurve::try_new_rational(
            4,
            vec![
                WeightedPoint3::try_new(p(2., -4.), 1.).unwrap(),
                WeightedPoint3::try_new(p(2., -4.), 1.).unwrap(),
                WeightedPoint3::try_new(p(2., (-8. - 2. * w) / (2. + w)), (2. + w) / 3.).unwrap(),
                WeightedPoint3::try_new(p(2., -2.), w).unwrap(),
                WeightedPoint3::try_new(p(4., -2.), 1.).unwrap(),
            ],
            vec![0., 0., 0., 0., 0., 1., 1., 1., 1., 1.],
        )
        .unwrap();
        assert_eq!(curve.derivative_at(0.).unwrap().length().unwrap(), 0.);
        assert!(
            curve
                .circular_center(Tolerance::DEFAULT)
                .unwrap()
                .unwrap()
                .distance_to(p(4., -4.))
                .unwrap()
                < 1e-10
        );
        for i in 0..=32 {
            assert!(
                (curve
                    .evaluate(i as Real / 32.)
                    .unwrap()
                    .distance_to(p(4., -4.))
                    .unwrap()
                    - 2.)
                    .abs()
                    < 1e-12
            );
        }
    }
}
