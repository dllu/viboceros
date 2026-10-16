use super::*;

fn sample(c: &NurbsCurve, density: u8, limit: usize) -> Result<CurvatureComb, GeometryError> {
    curvature_comb_with_subdivision(c, density, limit, &mut |c, interval| {
        c.try_trimmed(interval)
    })
}
fn quadratic() -> NurbsCurve {
    NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(1., -1.), p(2., 3.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}
#[test]
fn curvature_subdivision_keeps_hairs_sparse_and_outline_smooth() {
    let c = Circle3::try_from_center_point(
        p(0., 0.),
        p(2., 0.),
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    for (density, intervals) in [
        (0, 70),
        (1, 70),
        (2, 72),
        (3, 72),
        (4, 70),
        (5, 72),
        (6, 70),
        (7, 72),
        (8, 72),
        (9, 70),
        (10, 77),
    ] {
        let comb = sample(&c, density, 1000).unwrap();
        let polylines = comb.envelope_polylines(1., Tolerance::DEFAULT).unwrap();
        assert_eq!(polylines.len(), 1);
        assert_eq!(polylines[0].vertices().len(), 4 * intervals + 1);
        assert_eq!(
            comb.hairs.iter().filter(|h| h.display_hair).count(),
            4 * (usize::from(density) + 2)
        );
        for h in &comb.hairs {
            assert!((h.tip(1.).unwrap().distance_to(p(0., 0.)).unwrap() - 2.5).abs() < 1e-12);
        }
    }
}
#[test]
fn curvature_subdivision_follows_source_turns_without_changing_geometry() {
    let c = quadratic();
    let original = c.clone();
    let comb = sample(&c, 1, 1000).unwrap();
    assert_eq!(c, original);
    assert_eq!(comb.strips.len(), 3);
    for (range, (a, b)) in comb.strips.iter().zip([(0., 0.25), (0.25, 0.5), (0.5, 1.)]) {
        let hairs = &comb.hairs[range.clone()];
        assert_eq!(hairs.len(), 71);
        assert_eq!(hairs[0].parameter, a);
        assert_eq!(hairs.last().unwrap().parameter, b);
        for h in hairs {
            assert!(
                h.point
                    .distance_to(c.evaluate(h.parameter).unwrap())
                    .unwrap()
                    < 1e-13
            );
            let expected = CurveRef::NurbsCurve(&c)
                .curvature_vector(h.parameter)
                .unwrap()
                .to_array();
            let actual = h.curvature.to_array();
            assert!(
                (0..3)
                    .map(|i| (actual[i] - expected[i]).powi(2))
                    .sum::<Real>()
                    .sqrt()
                    < 1e-12
            );
        }
    }
    assert_eq!(
        comb.envelope_polylines(1., Tolerance::DEFAULT).unwrap()[0]
            .vertices()
            .len(),
        211
    );
    assert_eq!(
        sample(&c, 1, 212).unwrap_err(),
        GeometryError::CurvatureCombLimit
    );
    assert!(sample(&c, 1, 213).is_ok());
    assert_eq!(
        sample(&c, 11, 1000).unwrap_err(),
        GeometryError::CurvatureCombLimit
    );
}
#[test]
fn curvature_subdivision_retains_jumps_and_singular_gaps() {
    let c = NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(1., 1.), p(2., 0.), p(3., -1.), p(4., 3.)],
        vec![0., 0., 0., 1., 1., 2., 2., 2.],
    )
    .unwrap();
    let comb = sample(&c, 1, 1000).unwrap();
    let outlines = comb.envelope_polylines(1., Tolerance::DEFAULT).unwrap();
    assert_eq!(
        outlines
            .iter()
            .map(|p| p.vertices().len())
            .collect::<Vec<_>>(),
        vec![71, 211]
    );
    let cusp = NurbsCurve::try_new(
        2,
        vec![p(1., 0.), p(-1., 0.), p(1., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let comb = sample(&cusp, 1, 1000).unwrap();
    assert_eq!(comb.singular_samples, 2);
    assert_eq!(comb.strips.len(), 2);
    assert!(comb.hairs.iter().all(|h| h.parameter != 0.5));
    assert!(
        comb.envelope_polylines(1., Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn curvature_subdivision_is_independent_of_extreme_source_parameter_charts() {
    let c = quadratic();
    let baseline = sample(&c, 1, 1000).unwrap();
    for (a, b) in [(1e300, 1e300 + 1e287), (0., 1e-300), (-1e300, 1e300)] {
        let knots = vec![a, a, a, b, b, b];
        let changed = NurbsCurve::try_new_rational(2, c.control_points().to_vec(), knots).unwrap();
        let actual = sample(&changed, 1, 1000).unwrap();
        assert_eq!(actual.hairs.len(), baseline.hairs.len());
        for (a, b) in actual.hairs.iter().zip(&baseline.hairs) {
            assert_eq!(a.point, b.point);
            assert_eq!(a.curvature, b.curvature);
            assert_eq!(a.display_hair, b.display_hair);
        }
        assert_eq!(actual.domain, [a, b]);
        assert_eq!(actual.hairs.first().unwrap().parameter, a);
        assert_eq!(actual.hairs.last().unwrap().parameter, b);
    }
}
