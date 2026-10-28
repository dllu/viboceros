use super::*;

#[test]
fn a_single_polygon_profile_retains_its_corners_and_complete_image() {
    let curve = NurbsCurve::try_new(
        1,
        [[0., 0., 0.], [1., 2., 0.], [3., 1., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., 0., 0.5, 1., 1.],
    )
    .unwrap();
    let sections = vec![SweepSection {
        parameter: 2.5,
        curve,
    }];
    let before = sections.clone();
    let prepared = Sweep1::refit_sections(&sections, 0.001).unwrap();
    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].parameter, 2.5);
    assert!(
        sections[0]
            .curve
            .parameter_curve_deviation_bound(&prepared[0].curve, 1e-12)
            .unwrap()
            .is_some()
    );
    assert_eq!(sections, before);
}

#[test]
fn disabled_fits_align_regions_without_changing_weights_or_stations() {
    let curve = |points: &[[Real; 3]], weights: &[Real], knots| {
        NurbsCurve::try_new_rational(
            1,
            points
                .iter()
                .zip(weights)
                .map(|(&p, &w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
                .collect(),
            knots,
        )
        .unwrap()
    };
    let sections = vec![
        SweepSection {
            parameter: -2.,
            curve: curve(
                &[[0., 0., 0.], [1., 2., 0.], [3., 1., 0.]],
                &[2., 0.5, 4.],
                vec![10., 10., 12., 20., 20.],
            ),
        },
        SweepSection {
            parameter: 7.,
            curve: curve(
                &[[0., 0., 5.], [1., 2., 5.], [2., -0.3, 5.], [3., 1., 5.]],
                &[1., 3., 2., 5.],
                vec![-5., -5., -2., 3., 10., 10.],
            ),
        },
    ];
    let before = sections.clone();
    for tolerance in [0., 1e-9] {
        let prepared = Sweep1::refit_sections(&sections, tolerance).unwrap();
        assert_eq!(prepared.len(), 2);
        for (source, output) in sections.iter().zip(&prepared) {
            assert_eq!(source.parameter, output.parameter);
            assert_eq!(source.curve.degree(), output.curve.degree());
            assert_eq!(source.curve.control_points(), output.curve.control_points());
            assert_eq!(output.curve.domain(), 0. ..=1.);
        }
        // Both authored first corners now have the same correspondence. The
        // longer profile supplies the extra region; source knots need not match.
        assert_eq!(prepared[0].curve.knots()[2], prepared[1].curve.knots()[2]);
        assert_ne!(prepared[0].curve.knots()[2], 0.2);
        assert_eq!(sections, before);
    }
}
