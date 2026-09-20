use super::*;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn line(start: Point3, end: Point3) -> NurbsCurve {
    NurbsCurve::try_new(1, vec![start, end], vec![0., 0., 1., 1.]).unwrap()
}

#[test]
fn closest_curve_distant_projection_does_not_tie_with_the_first_endpoint() {
    let curve = line(p(0., 0., 0.), p(1., 0., 0.));
    let target = p(0.37, 1e100, 0.);
    assert_eq!(
        target.distance_to(p(0., 0., 0.)).unwrap(),
        target.distance_to(p(0.37, 0., 0.)).unwrap()
    );
    for domain in [0.0..=1.0, -2e12..=6e12, 0.0..=1e-200] {
        let curve = curve.try_reparameterized(domain).unwrap();
        let t = curve.closest_parameter(target, Tolerance::DEFAULT).unwrap();
        assert!(
            (curve.evaluate(t).unwrap().x() - 0.37).abs() < 1e-12,
            "t={t}"
        );
    }
}

#[test]
fn closest_curve_keeps_a_finite_minimum_when_every_distance_overflows() {
    let curve = line(p(0., 0., 0.), p(1., 1., 1.));
    let target = p(Real::MAX, Real::MAX, Real::MAX);
    assert!(target.distance_to(p(1., 1., 1.)).is_err());
    assert_eq!(
        curve.closest_parameter(target, Tolerance::DEFAULT).unwrap(),
        1.
    );
}

#[test]
fn closest_curve_projects_without_an_overflowing_displacement_vector() {
    let curve = line(p(Real::MAX, 0., 0.), p(Real::MAX, 1., 0.));
    let target = p(-Real::MAX, 0.37, 0.);
    assert!(curve.evaluate(0.).unwrap().vector_to(target).is_err());
    let t = curve.closest_parameter(target, Tolerance::DEFAULT).unwrap();
    assert!((t - 0.37).abs() < 1e-12, "t={t}");
    assert_eq!(curve.evaluate(t).unwrap(), p(Real::MAX, 0.37, 0.));
}

#[test]
fn closest_curve_normalizes_tangents_before_large_dot_products() {
    let scale = 1e200;
    let curve = line(p(0., 0., 0.), p(scale, 0., 0.));
    let target = p(0.37 * scale, scale, 0.);
    let t = curve.closest_parameter(target, Tolerance::DEFAULT).unwrap();
    assert!((t - 0.37).abs() < 1e-12, "t={t}");
    let curve = line(p(0., 0., 0.), p(Real::MAX, Real::MAX, Real::MAX));
    assert!(curve.derivative_at(0.5).unwrap().length().is_err());
    let t = curve
        .closest_parameter(
            p(0.37 * Real::MAX, 0.37 * Real::MAX, 0.37 * Real::MAX),
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert!((t - 0.37).abs() < 1e-12, "overflowing speed, t={t}");
}

#[test]
fn closest_curve_signed_poles_and_true_distance_ties_preserve_valid_minima() {
    for sign in [-1., 1.] {
        let curve = NurbsCurve::try_new_rational(
            1,
            vec![
                WeightedPoint3::try_new(p(0., 0., 0.), sign).unwrap(),
                WeightedPoint3::try_new(p(1., 0., 0.), -sign).unwrap(),
            ],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        assert_eq!(
            curve.evaluate(0.5),
            Err(GeometryError::ZeroWeightAtParameter)
        );
        for (x, expected_t) in [(-0.5, 0.25), (0.5, 0.)] {
            let t = curve
                .closest_parameter(p(x, 1e100, 0.), Tolerance::DEFAULT)
                .unwrap();
            assert_eq!(t, expected_t);
        }
    }
}

#[test]
fn closest_curve_translated_proposals_cannot_erase_the_original_target_offset() {
    let radius = 2_f64.powi(54);
    let curve = NurbsCurve::try_new_rational(
        2,
        [
            (-radius, 0., 1.),
            (-radius, radius, 0.5_f64.sqrt()),
            (0., radius, 1.),
            (radius, radius, 0.5_f64.sqrt()),
            (radius, 0., 1.),
        ]
        .into_iter()
        .map(|(x, y, w)| WeightedPoint3::try_new(p(x, y, 0.), w).unwrap())
        .collect(),
        vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.],
    )
    .unwrap();
    // Translating q by the first control rounds radius+1 to radius. That
    // creates a false symmetry in the proposal frame; the original target
    // strictly prefers the right endpoint of this upper semicircle.
    assert_eq!(radius + 1., radius);
    for (x, expected) in [(-1., 0.), (0., 0.), (1., 1.)] {
        let target = p(x, -radius, 0.);
        let t = curve.closest_parameter(target, Tolerance::DEFAULT).unwrap();
        assert_eq!(t, expected, "target x={x}");
    }
}
