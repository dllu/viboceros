use super::*;
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn surface() -> NurbsSurface {
    NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![p(0., 0., 0.), p(1., 0., 0.), p(0., 1., 0.), p(1., 1., 0.)],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}
fn curve(points: Vec<Point3>) -> NurbsCurve {
    let degree = points.len() - 1;
    NurbsCurve::try_new(
        degree,
        points,
        [vec![0.; degree + 1], vec![1.; degree + 1]].concat(),
    )
    .unwrap()
}
#[test]
fn external_contacts_require_valid_original_curve_and_surface_images() {
    let curve = curve(vec![p(0.5, 0.5, -1.), p(0.5, 0.5, 1.)]);
    let surface = surface();
    let before = (curve.clone(), surface.clone());
    let events = curve_surface_intersection_events_with_operation(
        &curve,
        &surface,
        Tolerance::DEFAULT,
        &mut |_, _, _| {
            Ok::<_, GeometryError>(Some(vec![CurveSurfaceIntersectionParameters {
                start: [0.5, 0.5, 0.5],
                end: None,
            }]))
        },
    )
    .unwrap();
    let [CurveSurfaceIntersectionEvent::Point(point)] = events.as_slice() else {
        panic!("expected point")
    };
    assert_eq!(point.point(), p(0.5, 0.5, 0.));
    for start in [
        [0., 0.5, 0.5],
        [1.5, 0.5, 0.5],
        [0.5, 1.5, 0.5],
        [0.5, 0.5, f64::NAN],
    ] {
        assert!(
            curve_surface_intersection_events_with_operation(
                &curve,
                &surface,
                Tolerance::DEFAULT,
                &mut |_, _, _| Ok::<_, GeometryError>(Some(vec![
                    CurveSurfaceIntersectionParameters { start, end: None }
                ]))
            )
            .is_err()
        );
    }
    assert_eq!((curve, surface), before);
}
#[test]
fn overlap_sampling_rejects_false_interiors_and_accepts_nonlinear_uv() {
    let surface = surface();
    for height in [0., 1.] {
        let source = curve(vec![p(0., 0.5, 0.), p(0., 0.5, height), p(1., 0.5, 0.)]);
        let result = curve_surface_intersection_events_with_operation(
            &source,
            &surface,
            Tolerance::DEFAULT,
            &mut |_, _, _| {
                Ok::<_, GeometryError>(Some(vec![CurveSurfaceIntersectionParameters {
                    start: [0., 0., 0.5],
                    end: Some([1., 1., 0.5]),
                }]))
            },
        );
        if height == 0. {
            assert!(matches!(
                result.unwrap().as_slice(),
                [CurveSurfaceIntersectionEvent::Overlap(_)]
            ));
        } else {
            assert!(result.is_err());
        }
    }
}
#[test]
fn event_merging_culls_covered_points_and_preserves_fallback_and_errors() {
    let source = curve(vec![p(0., 0.5, 0.), p(1., 0.5, 0.)]);
    let surface = surface();
    let result = curve_surface_intersection_events_with_operation(
        &source,
        &surface,
        Tolerance::DEFAULT,
        &mut |_, _, _| {
            Ok::<_, GeometryError>(Some(vec![
                CurveSurfaceIntersectionParameters {
                    start: [0.5, 0.5, 0.5],
                    end: Some([1., 1., 0.5]),
                },
                CurveSurfaceIntersectionParameters {
                    start: [0.5, 0.5, 0.5],
                    end: None,
                },
                CurveSurfaceIntersectionParameters {
                    start: [0.5, 0.5, 0.5],
                    end: Some([0., 0., 0.5]),
                },
            ]))
        },
    )
    .unwrap();
    let [CurveSurfaceIntersectionEvent::Overlap(overlap)] = result.as_slice() else {
        panic!("expected merged overlap")
    };
    assert_eq!(overlap.curve_interval(), 0. ..=1.);
    assert_eq!(
        curve_surface_intersection_events_with_operation(
            &source,
            &surface,
            Tolerance::DEFAULT,
            &mut |_, _, _| Ok::<_, GeometryError>(None)
        )
        .unwrap(),
        curve_surface_intersection_events(&source, &surface, Tolerance::DEFAULT).unwrap()
    );
    assert!(matches!(
        curve_surface_intersection_events_with_operation(
            &source,
            &surface,
            Tolerance::DEFAULT,
            &mut |_, _, _| Err::<Option<Vec<CurveSurfaceIntersectionParameters>>, _>(
                GeometryError::CurveIntersectionDidNotConverge
            )
        ),
        Err(GeometryError::CurveIntersectionDidNotConverge)
    ));
}
