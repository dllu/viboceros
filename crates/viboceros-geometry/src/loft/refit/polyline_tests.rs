use super::*;
use crate::Point3;

fn source(points: &[[Real; 3]], degree: usize, knots: Vec<Real>, weights: &[Real]) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        degree,
        points
            .iter()
            .zip(weights)
            .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(*p).unwrap(), *w).unwrap())
            .collect(),
        knots,
    )
    .unwrap()
}

#[test]
fn rational_and_unclamped_straight_regions_keep_their_exact_active_vertices() {
    for gauge in [1., -1.] {
        let c = source(
            &[
                [0., 0., 0.],
                [1., 0., 0.],
                [2., 0., 0.],
                [2., 1., 0.],
                [2., 2., 0.],
            ],
            2,
            vec![-1., -1., 0., 1., 1., 2., 3., 3.],
            &[gauge, 2. * gauge, gauge, 2. * gauge, gauge],
        );
        let before = c.clone();
        let fit = try_prepare_refitted_loft_sections(std::slice::from_ref(&c), 1e-12)
            .unwrap()
            .remove(0);
        assert_eq!(fit.control_points().len(), 7);
        assert_eq!(fit.degree(), 3);
        assert!(fit.control_points().iter().all(|c| c.weight() == 1.));
        let d = fit.domain();
        assert!(
            fit.evaluate(*d.start())
                .unwrap()
                .distance_to(Point3::try_new(2. / 3., 0., 0.).unwrap())
                .unwrap()
                < 1e-15
        );
        assert_eq!(
            fit.evaluate(fit.knots()[4]).unwrap(),
            Point3::try_new(2., 0., 0.).unwrap()
        );
        assert!(
            fit.evaluate(*d.end())
                .unwrap()
                .distance_to(Point3::try_new(2., 4. / 3., 0.).unwrap())
                .unwrap()
                < 1e-15
        );
        assert_eq!(c, before);
        assert!(try_prepare_refitted_loft_sections(&[c], 1e-300).is_err());
    }
}

#[test]
fn collinear_vertices_and_constant_spans_merge_but_reversals_remain() {
    let c = source(
        &[
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 0., 0.],
            [2., 0., 0.],
            [2., 1., 0.],
        ],
        1,
        vec![0., 0., 1., 2., 3., 4., 4.],
        &[1.; 5],
    );
    let fit = try_prepare_refitted_loft_sections(&[c], 1e-12)
        .unwrap()
        .remove(0);
    assert_eq!(fit.control_points().len(), 7);
    assert_eq!(fit.knots()[4], 2.);
    let c = source(
        &[[0., 0., 0.], [2., 0., 0.], [1., 0., 0.], [1., 1., 0.]],
        1,
        vec![0., 0., 1., 2., 3., 3.],
        &[1.; 4],
    );
    let fit = try_prepare_refitted_loft_sections(&[c], 1e-12)
        .unwrap()
        .remove(0);
    assert_eq!(fit.control_points().len(), 10);
    assert_eq!(
        fit.evaluate(fit.knots()[4]).unwrap(),
        Point3::try_new(2., 0., 0.).unwrap()
    );
    assert_eq!(
        fit.evaluate(fit.knots()[7]).unwrap(),
        Point3::try_new(1., 0., 0.).unwrap()
    );
}

#[test]
fn shallow_bends_and_unrepresentable_or_oversized_piece_layouts_fail_explicitly() {
    for degrees in [0.5_f64, 1., 1.001] {
        let a = degrees.to_radians();
        let c = source(
            &[[0., 0., 0.], [1., 0., 0.], [1. + a.cos(), a.sin(), 0.]],
            1,
            vec![0., 0., 1., 2., 2.],
            &[1.; 3],
        );
        assert_eq!(
            try_prepare_refitted_loft_sections(&[c], 0.001).is_ok(),
            degrees > 1.
        );
    }
    let points = (0..=171)
        .map(|i| [i as Real, (i % 2) as Real, 0.])
        .collect::<Vec<_>>();
    let c = source(
        &points,
        1,
        [vec![0.], (0..=171).map(|i| i as Real).collect(), vec![171.]].concat(),
        &vec![1.; 172],
    );
    assert!(matches!(
        try_prepare_refitted_loft_sections(&[c], 0.001),
        Err(GeometryError::LoftResourceLimit {
            context: "Refit polynomial section controls",
            maximum: 512
        })
    ));
}
