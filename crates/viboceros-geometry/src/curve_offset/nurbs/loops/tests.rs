use super::*;

fn ellipse() -> NurbsCurve {
    crate::Ellipse3::try_from_three_points(
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(6., 0., 0.).unwrap(),
        Point3::try_new(0., 4., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}
#[test]
fn cusp_cycles_select_disjoint_minimal_ears_even_when_their_complements_are_negative() {
    let arcs = vec![[0.42, 0.58], [0.92, 1.08], [0.58, 1.42], [0.08, 0.92]];
    assert_eq!(
        cycles::minimal(arcs).unwrap(),
        vec![[0.42, 0.58], [0.92, 1.08]]
    );
    assert_eq!(
        cycles::minimal(vec![[0.1, 0.8], [0.2, 0.3], [0.4, 0.5]]).unwrap(),
        vec![[0.2, 0.3], [0.4, 0.5]]
    );
}
#[test]
fn cusp_polynomial_area_is_oriented_and_translation_independent() {
    let points = [
        [0., 0., 0.],
        [2., 0., 0.],
        [2., 3., 0.],
        [0., 3., 0.],
        [0., 0., 0.],
    ]
    .map(|p| Point3::try_from(p).unwrap());
    let curve = Polyline3::try_new(points.to_vec(), Tolerance::DEFAULT)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let normal = Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    assert!((cycles::area(&curve, normal).unwrap() - 6.).abs() < 1e-14);
    assert!((cycles::area(&curve.reversed().unwrap(), normal).unwrap() + 6.).abs() < 1e-14);
    let translated = curve
        .transformed(crate::AffineTransform3::from_translation(
            Vector3::try_new(1e9, -2e9, 3e9).unwrap(),
        ))
        .unwrap();
    assert!((cycles::area(&translated, normal).unwrap() - 6.).abs() < 1e-14);
}

#[test]
fn cusp_backward_certificate_respects_rational_source_tangents() {
    let point = |x| Point3::try_new(x, 7., -3.).unwrap();
    let source = NurbsCurve::try_new_rational(
        2,
        [(1e6, 1.), (1e6 + 1., 0.5), (1e6 + 2., 2.)]
            .map(|(x, w)| WeightedPoint3::try_new(point(x), w).unwrap())
            .to_vec(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let line =
        NurbsCurve::try_new(1, vec![point(1e6), point(1e6 + 2.)], vec![0., 0., 1., 1.]).unwrap();
    assert!(!forward::backward(&line, &source).unwrap());
    let reversed = line
        .reversed()
        .unwrap()
        .try_reparameterized(line.domain())
        .unwrap();
    assert!(forward::backward(&reversed, &source).unwrap());
    let mixed = NurbsCurve::try_new(
        3,
        vec![
            point(1e6),
            point(1e6 - 1.),
            point(1e6 + 1.),
            point(1e6 + 2.),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    assert!(!forward::backward(&mixed, &source).unwrap());
}
#[test]
fn cusp_ellipse_intersections_match_the_independent_normal_equations() {
    let source = ellipse();
    let before = source.clone();
    let normal = Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let tolerance = Tolerance::try_new(2.5e-7, 1e-12, 1e-6).unwrap();
    for distance in [2.67, 3., 3.99] {
        let curve = offset_closed_region(&source, distance, normal, tolerance, None).unwrap();
        let sine_squared = ((distance * 6. / 4.).powi(2) - 16.) / 20.;
        let x = (10. / 3.) * (1. - sine_squared).sqrt();
        let start = curve.evaluate(*curve.domain().start()).unwrap();
        assert!(
            start
                .distance_to(Point3::try_new(x, 0., 0.).unwrap())
                .unwrap()
                < 1e-7,
            "{distance}: {start:?}"
        );
        assert!(curve.is_closed().unwrap());
    }
    assert_eq!(source, before);
}

#[test]
fn cusp_support_width_rejects_collapse_without_rejecting_nearby_regions_or_outward_offsets() {
    let source = ellipse();
    let normal = Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    for distance in [4., 5.] {
        assert!(limits::collapsed(&source, normal, distance, Tolerance::DEFAULT).unwrap());
        assert!(matches!(
            offset_closed_region(&source, distance, normal, Tolerance::DEFAULT, None),
            Err(GeometryError::DegenerateOffsetRegion)
        ));
    }
    for distance in [-5., 0., 3.99] {
        assert!(!limits::collapsed(&source, normal, distance, Tolerance::DEFAULT).unwrap());
    }
}

#[test]
fn cusp_trimming_retains_a_surviving_stored_seam() {
    let source = ellipse();
    let seam = source.parameter_at(0.125).unwrap();
    let source = source.try_change_closed_seam(seam).unwrap();
    let normal = Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let tolerance = Tolerance::try_new(2.5e-7, 1e-12, 1e-6).unwrap();
    let expected = offset_sample(
        &source,
        3.,
        normal,
        *source.domain().start(),
        ParameterSide::Right,
    )
    .unwrap()
    .point;
    let result = offset_closed_region(&source, 3., normal, tolerance, None).unwrap();
    assert!(
        result
            .evaluate(*result.domain().start())
            .unwrap()
            .distance_to(expected)
            .unwrap()
            < 1e-9
    );
    assert_eq!(
        result
            .full_multiplicity_kink_parameters(Tolerance::DEFAULT)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn cusp_reversed_traversal_and_negative_weight_gauge_keep_the_analytic_tip() {
    let source = ellipse();
    let negative = NurbsCurve::try_new_rational(
        source.degree(),
        source
            .control_points()
            .iter()
            .map(|c| WeightedPoint3::try_new(c.point(), -c.weight()).unwrap())
            .collect(),
        source.knots().to_vec(),
    )
    .unwrap();
    let normal = Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let tolerance = Tolerance::try_new(2.5e-7, 1e-12, 1e-6).unwrap();
    for (source, distance, orientation) in
        [(source.reversed().unwrap(), -3., -1.), (negative, 3., 1.)]
    {
        let before = source.clone();
        let curve = offset_closed_region(&source, distance, normal, tolerance, None).unwrap();
        let x = (10. / 3.) * (1.0_f64 - ((4.5_f64).powi(2) - 16.) / 20.).sqrt();
        assert!(
            curve
                .evaluate(*curve.domain().start())
                .unwrap()
                .distance_to(Point3::try_new(x, 0., 0.).unwrap())
                .unwrap()
                < 1e-7
        );
        assert!(cycles::area(&curve, normal).unwrap() * orientation > 0.);
        assert_eq!(source, before);
    }
}
