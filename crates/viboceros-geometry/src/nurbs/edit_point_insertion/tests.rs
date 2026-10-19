use super::*;
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn source() -> NurbsCurve {
    NurbsCurve::try_new_rational(
        3,
        vec![
            (p(0., 0., 0.), 1.),
            (p(1., 3., 2.), 2.),
            (p(3., -1., -2.), 3.),
            (p(5., 0., 1.), 1.),
        ]
        .into_iter()
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
}
#[test]
fn edit_point_refinement_preserves_spatial_rational_locus_and_large_translated_charts() {
    let c = source();
    let wide = NurbsCurve::try_new_rational(
        c.degree(),
        c.control_points().to_vec(),
        vec![
            -Real::MAX,
            -Real::MAX,
            -Real::MAX,
            -Real::MAX,
            Real::MAX,
            Real::MAX,
            Real::MAX,
            Real::MAX,
        ],
    )
    .unwrap();
    for c in [
        c.clone(),
        c.try_reparameterized(1e307..=1.4e307).unwrap(),
        wide,
    ] {
        let domain = c.domain();
        let at = |fraction: Real| (1. - fraction) * domain.start() + fraction * domain.end();
        let t = at(0.35);
        let edited = c.try_insert_edit_point(t).unwrap();
        assert_eq!(edited.control_points().len(), c.control_points().len() + 1);
        assert_eq!(edited.degree(), c.degree());
        assert_eq!(edited.domain(), domain);
        for i in 0..=100 {
            let u = at(i as Real / 100.);
            assert!(
                c.evaluate(u)
                    .unwrap()
                    .distance_to(edited.evaluate(u).unwrap())
                    .unwrap()
                    < 1e-12
            );
        }
    }
}
#[test]
fn edit_point_seam_clamping_keeps_the_curve_and_admits_active_greville_markers() {
    let c = NurbsCurve::try_control_point_curve_with_closure(
        3,
        vec![p(0., 0., 0.), p(1., 3., 1.), p(3., -1., 0.), p(5., 0., 2.)],
        ControlPointCurveClosure::Smooth,
    )
    .unwrap();
    let domain = c.domain();
    let edited = c.try_insert_edit_point(*domain.start()).unwrap();
    assert!(
        !edited
            .greville_points_in_active_domain()
            .unwrap()
            .is_empty()
    );
    for i in 0..=100 {
        let u =
            crate::parameter::map_parameter(i as Real / 100., 0.0..=1.0, domain.clone()).unwrap();
        assert!(
            c.evaluate(u)
                .unwrap()
                .distance_to(edited.evaluate(u).unwrap())
                .unwrap()
                < 1e-12
        );
    }
    let near = *domain.start() + 0.01 * (*domain.end() - *domain.start());
    assert_eq!(c.try_insert_edit_point(near).unwrap(), c);
}
#[test]
fn edit_point_invalid_parameters_and_unrepresentable_pole_controls_preserve_sources() {
    let c = source();
    let original = c.clone();
    for u in [Real::NAN, Real::INFINITY, -1., 2.] {
        assert!(c.try_insert_edit_point(u).is_err());
        assert_eq!(c, original);
    }
    let pole = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(0., 0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(1., 0., 0.), -1.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let original = pole.clone();
    assert!(pole.try_insert_edit_point(0.5).is_err());
    assert_eq!(pole, original);
}
