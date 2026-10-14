use super::*;
use crate::CurveCurveIntersection;

fn uv(a: [f64; 2], b: [f64; 2], domain: [f64; 2]) -> NurbsCurve2 {
    NurbsCurve2::try_new(
        1,
        vec![Point2::try_from(a).unwrap(), Point2::try_from(b).unwrap()],
        vec![domain[0], domain[0], domain[1], domain[1]],
    )
    .unwrap()
}

#[test]
fn clipping_preserves_independent_xyz_and_uv_parameter_charts() {
    let source = surface();
    let face = Brep::try_rectangular_surface_face(
        source.clone(),
        0.25..=0.75,
        0. ..=1.,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let spatial = NurbsCurve::try_new(
        1,
        vec![p(0., 0.5, 0.), p(1., 0.5, 0.)],
        vec![-4., -4., 9., 9.],
    )
    .unwrap();
    let first_uv = uv([0., 0.5], [1., 0.5], [10., 16.]);
    let second_uv = uv([0., 0.5], [1., 0.5], [-9., 22.]);
    let before = face.clone();
    let events = surface_face_intersection_events_with_operations(
        SurfaceIntersectionInput::Surface(&source),
        SurfaceIntersectionInput::Face(&face.faces()[0]),
        Tolerance::DEFAULT,
        &mut |_, _, _| {
            Ok::<_, GeometryError>(Some(vec![SurfaceSurfaceIntersectionProposal::Curve {
                spatial: spatial.clone(),
                first_uv: first_uv.clone(),
                second_uv: second_uv.clone(),
            }]))
        },
        &mut |a, b, tol| a.intersection_events_with_curve(b, tol),
    )
    .unwrap()
    .unwrap();
    let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
        panic!("{events:?}");
    };
    assert_eq!(curve.domain(), -0.75..=5.75);
    assert!(
        curve
            .evaluate(-0.75)
            .unwrap()
            .distance_to(p(0.25, 0.5, 0.))
            .unwrap()
            < 1e-12
    );
    assert!(
        curve
            .evaluate(5.75)
            .unwrap()
            .distance_to(p(0.75, 0.5, 0.))
            .unwrap()
            < 1e-12
    );
    assert_eq!(face, before);
}

#[test]
fn corner_contacts_and_boundary_overlaps_survive_trim_interval_classification() {
    let source = surface();
    let diagonal =
        NurbsSurface::try_bilinear([p(0., 1., -1.), p(1., 0., -1.), p(1., 0., 1.), p(0., 1., 1.)])
            .unwrap();
    let face = Brep::try_rectangular_surface_face(
        source.clone(),
        0. ..=0.5,
        0. ..=0.5,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let spatial =
        NurbsCurve::try_new(1, vec![p(0., 1., 0.), p(1., 0., 0.)], vec![0., 0., 1., 1.]).unwrap();
    let events = surface_face_intersection_events_with_operations(
        SurfaceIntersectionInput::Face(&face.faces()[0]),
        SurfaceIntersectionInput::Surface(&diagonal),
        Tolerance::DEFAULT,
        &mut |_, _, _| {
            Ok::<_, GeometryError>(Some(vec![SurfaceSurfaceIntersectionProposal::Curve {
                spatial: spatial.clone(),
                first_uv: uv([0., 1.], [1., 0.], [0., 1.]),
                second_uv: uv([0., 0.5], [1., 0.5], [0., 1.]),
            }]))
        },
        &mut |a, b, tol| a.intersection_events_with_curve(b, tol),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        events,
        vec![SurfaceSurfaceIntersectionEvent::Point(p(0.5, 0.5, 0.))]
    );

    let face = Brep::try_rectangular_surface_face(
        source.clone(),
        0.25..=0.75,
        0.5..=1.,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let events = surface_face_intersection_events_with_operations(
        SurfaceIntersectionInput::Surface(&source),
        SurfaceIntersectionInput::Face(&face.faces()[0]),
        Tolerance::DEFAULT,
        &mut |_, _, _| {
            Ok::<_, GeometryError>(Some(vec![SurfaceSurfaceIntersectionProposal::Curve {
                spatial: NurbsCurve::try_new(
                    1,
                    vec![p(0., 0.5, 0.), p(1., 0.5, 0.)],
                    vec![0., 0., 1., 1.],
                )
                .unwrap(),
                first_uv: uv([0., 0.5], [1., 0.5], [0., 1.]),
                second_uv: uv([0., 0.5], [1., 0.5], [0., 1.]),
            }]))
        },
        &mut |a, b, tol| a.intersection_events_with_curve(b, tol),
    )
    .unwrap()
    .unwrap();
    let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
        panic!("{events:?}");
    };
    assert_eq!(curve.domain(), 0.25..=0.75);
}

#[test]
fn false_uv_boundary_contacts_and_intersector_errors_fail_before_output() {
    let source = surface();
    let face = Brep::try_rectangular_surface_face(
        source.clone(),
        0.25..=0.75,
        0. ..=1.,
        Tolerance::DEFAULT,
    )
    .unwrap();
    for fail in [false, true] {
        let result = surface_face_intersection_events_with_operations(
            SurfaceIntersectionInput::Surface(&source),
            SurfaceIntersectionInput::Face(&face.faces()[0]),
            Tolerance::DEFAULT,
            &mut |_, _, _| {
                Ok::<_, GeometryError>(Some(vec![SurfaceSurfaceIntersectionProposal::Curve {
                    spatial: NurbsCurve::try_new(
                        1,
                        vec![p(0., 0.5, 0.), p(1., 0.5, 0.)],
                        vec![0., 0., 1., 1.],
                    )
                    .unwrap(),
                    first_uv: uv([0., 0.5], [1., 0.5], [0., 1.]),
                    second_uv: uv([0., 0.5], [1., 0.5], [0., 1.]),
                }]))
            },
            &mut |a, b, _| {
                if fail {
                    return Err(GeometryError::CurveIntersectionDidNotConverge);
                }
                Ok(vec![crate::CurveCurveIntersectionEvent::Point(
                    CurveCurveIntersection {
                        first_parameter: a.parameter_at(0.5).unwrap(),
                        second_parameter: *b.domain().start(),
                        point: p(0.5, 0.5, 0.),
                    },
                )])
            },
        );
        assert!(matches!(
            result,
            Err(GeometryError::CurveIntersectionDidNotConverge)
        ));
    }
}
