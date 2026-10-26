use super::*;
use crate::{CurveContinuity, Point3};

fn source(degree: usize, points: &[[Real; 2]], knots: Vec<Real>) -> NurbsCurve {
    NurbsCurve::try_new(
        degree,
        points
            .iter()
            .map(|p| Point3::try_new(p[0], p[1], 0.).unwrap())
            .collect(),
        knots,
    )
    .unwrap()
}

#[test]
fn unresolved_bends_and_continuous_tangent_curvature_changes_have_complete_bounds() {
    for c in [
        source(
            1,
            &[[0., 0.], [1., 0.], [2., 1e-5]],
            vec![0., 0., 1., 2., 2.],
        ),
        source(
            3,
            &[
                [0., 0.],
                [0.25, 0.],
                [0.75, 0.],
                [1.25, 1e-5],
                [1.75, 2e-5],
                [2., 2e-5],
            ],
            [vec![0.; 4], vec![0.5; 2], vec![1.; 4]].concat(),
        ),
    ] {
        let sources = vec![c.clone(); 2];
        let before = sources.clone();
        let fits = try_prepare_refitted_loft_sections(&sources, 0.001).unwrap();
        assert!(
            c.discontinuity_parameters(CurveContinuity::G1, 1_f64.to_radians())
                .unwrap()
                .is_empty()
        );
        for fit in fits {
            assert_eq!((fit.degree(), fit.control_points().len()), (3, 4));
            let pairs = (0..=64)
                .map(|i| {
                    let t = i as Real / 64.;
                    [t, c.evaluate(c.parameter_at(t).unwrap()).unwrap().x() / 2.]
                })
                .collect::<Vec<_>>();
            assert!(
                c.mapped_curve_deviation_bound(&fit, &pairs, 0.0005)
                    .unwrap()
                    .is_some()
            );
        }
        assert!(try_prepare_refitted_loft_sections(&sources, 1e-6).is_err());
        assert_eq!(sources, before);
    }
}

#[test]
fn a_positional_bound_cannot_erase_a_resolved_corner_or_a_jump() {
    let corner = source(
        1,
        &[[0., 0.], [1., 0.], [1., 1e-5], [2., 1e-5]],
        vec![0., 0., 1., 2., 3., 3.],
    );
    let initial = crate::curve_fit::g1::initial_cubic(&corner).unwrap();
    let pairs = [
        [0., 0.],
        [1. / 3., 0.5 - 1e-5],
        [2. / 3., 0.5 + 1e-5],
        [1., 1.],
    ];
    assert!(
        corner
            .mapped_curve_deviation_bound(&initial, &pairs, 0.0005)
            .unwrap()
            .is_some()
    );
    assert!(prepare(&[corner.clone(), corner.clone()], 0.001, None).is_err());
    let fits = try_prepare_refitted_loft_sections(&[corner.clone(), corner], 0.001).unwrap();
    assert_eq!(fits[0].control_points().len(), 10);
    let jump = source(
        1,
        &[[0., 0.], [1., 0.], [1., 1e-5], [2., 1e-5]],
        vec![0., 0., 1., 1., 2., 2.],
    );
    let before = jump.clone();
    assert!(try_prepare_refitted_loft_sections(&[jump.clone(), jump.clone()], 0.001).is_err());
    assert_eq!(jump, before);
}

#[test]
fn multispan_interior_bumps_cannot_hide_behind_endpoints_and_the_midpoint() {
    let ys = [0., 0., 13., -13., 0., 0., 0., 13., -13., 0., 0.];
    let points = ys
        .into_iter()
        .enumerate()
        .map(|(i, y)| [i as Real, y])
        .collect::<Vec<_>>();
    let c = source(
        5,
        &points,
        [vec![0.; 6], vec![0.5; 5], vec![1.; 6]].concat(),
    );
    let initial = crate::curve_fit::g1::initial_cubic(&c).unwrap();
    for t in [0., 0.5, 1.] {
        assert!(
            c.evaluate(t)
                .unwrap()
                .distance_to(initial.evaluate(t).unwrap())
                .unwrap()
                < 1e-14
        );
    }
    assert!(
        c.evaluate(0.125)
            .unwrap()
            .distance_to(initial.evaluate(0.125).unwrap())
            .unwrap()
            > 1.
    );
    assert!(
        c.discontinuity_parameters(CurveContinuity::G1, 1_f64.to_radians())
            .unwrap()
            .is_empty()
    );
    let sources = vec![c; 2];
    let before = sources.clone();
    assert!(try_prepare_refitted_loft_sections(&sources, 0.1).is_err());
    assert_eq!(sources, before);
}
