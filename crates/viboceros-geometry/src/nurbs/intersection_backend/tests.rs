use super::*;
fn curve(points: &[[Real; 2]]) -> NurbsCurve {
    let degree = points.len() - 1;
    NurbsCurve::try_new(
        degree,
        points
            .iter()
            .map(|p| Point3::try_new(p[0], p[1], 0.).unwrap())
            .collect(),
        [vec![0.; degree + 1], vec![1.; degree + 1]].concat(),
    )
    .unwrap()
}
#[test]
fn external_contacts_are_checked_on_both_original_curves_and_ordered() {
    let a = curve(&[[0., 0.], [1., 2.], [2., 0.]]);
    let b = curve(&[[0., 0.5], [2., 0.5]]);
    let lo = (1. - 0.5_f64.sqrt()) * 0.5;
    let hi = 1. - lo;
    let result = a
        .intersection_events_with_curve_with_operation(&b, Tolerance::DEFAULT, &mut |_, _, _| {
            Ok::<_, GeometryError>(Some(vec![
                CurveCurveIntersectionParameters {
                    start: [hi, hi],
                    end: None,
                },
                CurveCurveIntersectionParameters {
                    start: [lo, lo],
                    end: None,
                },
                CurveCurveIntersectionParameters {
                    start: [lo, lo],
                    end: None,
                },
            ]))
        })
        .unwrap();
    assert_eq!(result.len(), 2);
    let CurveCurveIntersectionEvent::Point(first) = result[0] else {
        panic!()
    };
    assert!((first.first_parameter() - lo).abs() < 1e-14);
    assert!(
        first
            .point()
            .distance_to(Point3::try_new(2. * lo, 0.5, 0.).unwrap())
            .unwrap()
            < 1e-14
    );
    for start in [[0.5, 0.5], [0., 0.5], [-1., 0.5], [f64::NAN, 0.5]] {
        assert!(
            a.intersection_events_with_curve_with_operation(
                &b,
                Tolerance::DEFAULT,
                &mut |_, _, _| Ok::<_, GeometryError>(Some(vec![
                    CurveCurveIntersectionParameters { start, end: None }
                ]))
            )
            .is_err()
        );
    }
    let error = GeometryError::InterpolationBackendFailed {
        message: "intersection producer".into(),
    };
    assert_eq!(
        a.intersection_events_with_curve_with_operation(
            &b,
            Tolerance::DEFAULT,
            &mut |_, _, _| Err::<Option<Vec<CurveCurveIntersectionParameters>>, _>(error.clone())
        ),
        Err(error)
    );
    assert_eq!(
        a.intersection_events_with_curve_with_operation(
            &b,
            Tolerance::DEFAULT,
            &mut |_, _, _| Ok::<_, GeometryError>(None)
        )
        .unwrap(),
        a.intersection_events_with_curve(&b, Tolerance::DEFAULT)
            .unwrap()
    );
}
#[test]
fn shared_overlap_checks_accept_reversed_nonlinear_parameter_maps_and_reject_false_ranges() {
    let a = curve(&[[0., 0.], [2., 0.]]);
    let b = curve(&[[2., 0.], [2., 0.], [0., 0.]]);
    let overlap = CurveCurveIntersectionParameters {
        start: [0., 1.],
        end: Some([1., 0.]),
    };
    let result = a
        .intersection_events_with_curve_with_operation(&b, Tolerance::DEFAULT, &mut |_, _, _| {
            Ok::<_, GeometryError>(Some(vec![overlap]))
        })
        .unwrap();
    assert!(matches!(
        result.as_slice(),
        [CurveCurveIntersectionEvent::Overlap(_)]
    ));
    let bent = curve(&[[0., 0.], [1., 2.], [2., 0.]]);
    assert!(
        a.intersection_events_with_curve_with_operation(
            &bent,
            Tolerance::DEFAULT,
            &mut |_, _, _| Ok::<_, GeometryError>(Some(vec![CurveCurveIntersectionParameters {
                start: [0., 0.],
                end: Some([1., 1.])
            }]))
        )
        .is_err()
    );
}
