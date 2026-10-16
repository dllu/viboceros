use super::*;
use crate::{LineSegment, ParameterSide, Polyline3, WeightedPoint3};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn vector_distance(a: crate::Vector3, b: crate::Vector3) -> Real {
    (a.x() - b.x()).hypot(a.y() - b.y()).hypot(a.z() - b.z())
}

fn smooth() -> NurbsCurve {
    NurbsCurve::try_new_rational(
        3,
        [
            (p(0., 0., 0.), 1.),
            (p(1., 2., 3.), 0.7),
            (p(3., -1., 1.), 1.2),
            (p(4., 1., 2.), 1.),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .to_vec(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
}

#[test]
fn constrained_cubic_fits_interpolate_unsorted_adjacent_stations_and_remain_c2() {
    for source in [
        smooth(),
        smooth()
            .reversed()
            .unwrap()
            .try_reparameterized(-13. ..=29.)
            .unwrap(),
    ] {
        let before = source.clone();
        let pins = [0.79, 0., 0.321, 0.32, 1., 0.07].map(|fraction| {
            let t = source.parameter_at(fraction).unwrap();
            CurveFitConstraint {
                source_parameter: t,
                point: source.evaluate(t).unwrap(),
            }
        });
        let fit = try_fit_cubic_curve_with_constraints(
            CurveRef::NurbsCurve(&source),
            &pins,
            1e-6,
            0.,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(fit.constraint_parameters.len(), pins.len());
        for (pin, &t) in pins.iter().zip(&fit.constraint_parameters) {
            assert!(
                fit.curve
                    .evaluate(t)
                    .unwrap()
                    .distance_to(pin.point)
                    .unwrap()
                    < 1e-11
            );
            if t > 0. && t < *fit.curve.domain().end() {
                assert_eq!(fit.curve.knots().iter().filter(|&&k| k == t).count(), 1);
                let a = fit
                    .curve
                    .evaluate_with_second_derivative_on_side(t, ParameterSide::Left)
                    .unwrap();
                let b = fit
                    .curve
                    .evaluate_with_second_derivative_on_side(t, ParameterSide::Right)
                    .unwrap();
                assert!(a.0.distance_to(b.0).unwrap() < 1e-11);
                assert!(vector_distance(a.1, b.1) < 1e-9);
                assert!(vector_distance(a.2, b.2) < 1e-7);
            }
        }
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&source), Tolerance::DEFAULT).unwrap();
        for i in 0..=513 {
            let t = sampler.total_length() * i as Real / 513.;
            assert!(
                fit.curve
                    .evaluate(t)
                    .unwrap()
                    .distance_to(sampler.point_at_distance(t).unwrap())
                    .unwrap()
                    < 1.1e-6
            );
        }
        assert_eq!(source, before);
    }
}

#[test]
fn constrained_fit_can_move_an_interior_target_within_the_fit_budget() {
    let line = LineSegment::try_new(p(0., 0., 0.), p(3., 0., 0.), Tolerance::DEFAULT).unwrap();
    let target = p(1.5, 1e-5, 0.);
    let fit = try_fit_cubic_curve_with_constraints(
        CurveRef::Line(&line),
        &[CurveFitConstraint {
            source_parameter: 1.5,
            point: target,
        }],
        1e-3,
        0.,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!(
        fit.curve
            .evaluate(1.5)
            .unwrap()
            .distance_to(target)
            .unwrap()
            < 1e-12
    );
    assert_eq!(fit.curve.evaluate(0.).unwrap(), line.start());
    assert_eq!(fit.curve.evaluate(3.).unwrap(), line.end());
}

#[test]
fn constrained_fit_preserves_source_kinks_and_accepts_pins_on_both_sides() {
    let source = Polyline3::try_new(
        vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 3., 0.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let pins = [0.9, 1., 1.1].map(|t| CurveFitConstraint {
        source_parameter: t,
        point: CurveRef::Polyline(&source).evaluate(t).unwrap(),
    });
    let fit = try_fit_cubic_curve_with_constraints(
        CurveRef::Polyline(&source),
        &pins,
        1e-7,
        0.1,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(fit.curve.knots().iter().filter(|&&k| k == 2.).count(), 3);
    for (pin, t) in pins.iter().zip(fit.constraint_parameters) {
        assert!(
            fit.curve
                .evaluate(t)
                .unwrap()
                .distance_to(pin.point)
                .unwrap()
                < 1e-12
        );
    }
    assert!(matches!(
        try_fit_cubic_curve_with_constraints(
            CurveRef::Polyline(&source),
            &[CurveFitConstraint {
                source_parameter: 1.,
                point: p(2., 1e-5, 0.)
            }],
            1e-3,
            0.1,
            Tolerance::DEFAULT,
        ),
        Err(GeometryError::InvalidCurveFitConstraints { .. })
    ));
    let (_, incoming) = fit
        .curve
        .evaluate_with_derivative_on_side(2., ParameterSide::Left)
        .unwrap();
    let (_, outgoing) = fit
        .curve
        .evaluate_with_derivative_on_side(2., ParameterSide::Right)
        .unwrap();
    assert!(vector_distance(incoming, crate::Vector3::try_new(1., 0., 0.).unwrap()) < 1e-12);
    assert!(vector_distance(outgoing, crate::Vector3::try_new(0., 1., 0.).unwrap()) < 1e-12);
}

#[test]
fn constrained_fit_rejects_ambiguous_stations_and_invalid_targets() {
    let source = smooth();
    let pin = CurveFitConstraint {
        source_parameter: 0.5,
        point: source.evaluate(0.5).unwrap(),
    };
    let fit = |pins: &[CurveFitConstraint]| {
        try_fit_cubic_curve_with_constraints(
            CurveRef::NurbsCurve(&source),
            pins,
            1e-3,
            0.,
            Tolerance::DEFAULT,
        )
    };
    for pins in [
        vec![pin; 2],
        vec![pin; 65],
        vec![CurveFitConstraint {
            point: p(100., 0., 0.),
            ..pin
        }],
        vec![CurveFitConstraint {
            source_parameter: 0.,
            point: p(0., 1e-5, 0.),
        }],
    ] {
        assert!(matches!(
            fit(&pins),
            Err(GeometryError::InvalidCurveFitConstraints { .. })
        ));
    }
    for source_parameter in [-0.1, 1.1, Real::NAN, Real::INFINITY] {
        assert!(matches!(
            fit(&[CurveFitConstraint {
                source_parameter,
                ..pin
            }]),
            Err(GeometryError::ParameterOutOfDomain { .. })
        ));
    }
}

#[test]
fn constrained_fit_rejects_an_inaccurate_external_solution_even_if_sampled_fit_passes() {
    let source = LineSegment::try_new(p(0., 0., 0.), p(3., 0., 0.), Tolerance::DEFAULT).unwrap();
    let result = try_fit_cubic_curve_with_constraints_with_solver(
        CurveRef::Line(&source),
        &[CurveFitConstraint {
            source_parameter: 1.5,
            point: p(1.5, 0., 0.),
        }],
        0.1,
        0.,
        Tolerance::DEFAULT,
        &mut |input| -> Result<Vec<Real>, GeometryError> {
            let mut solution = input.system.targets.to_vec();
            for (i, &fixed) in input.fixed_rows.iter().enumerate() {
                if fixed == 0 {
                    solution[4 * i] += 0.01;
                }
            }
            Ok(solution)
        },
    );
    assert!(matches!(
        result,
        Err(GeometryError::CurveFitConstraintDidNotConverge { .. })
    ));
}
