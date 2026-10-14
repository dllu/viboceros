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
#[test]
fn single_surface_clip_preserves_curved_holes_and_rejects_a_wrong_spatial_image() {
    use crate::{BrepFace, Frame3, Polyline3, Vector3};
    let rectangle = |lo: f64, hi: f64| {
        Polyline3::try_new(
            vec![
                p(lo, lo, 0.),
                p(hi, lo, 0.),
                p(hi, hi, 0.),
                p(lo, hi, 0.),
                p(lo, lo, 0.),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let planar = Brep::try_planar_face_with_holes(
        &rectangle(0., 1.),
        &[rectangle(0.4, 0.6)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let original = &planar.faces()[0];
    let surface = NurbsSurface::try_cylinder(
        Frame3::try_from_normal(
            p(0., 0., 0.),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
        2.,
        0.,
        1.,
    )
    .unwrap()
    .try_reparameterized(original.surface().domain_u(), original.surface().domain_v())
    .unwrap();
    let face = BrepFace::try_new(surface, false, original.loops().to_vec()).unwrap();
    let u = face.surface().domain_u();
    let v = face.surface().domain_v();
    let mid = 0.5 * (*v.start() + *v.end());
    let spatial = face.surface().isocurve_u(mid).unwrap();
    let witness = uv([*u.start(), mid], [*u.end(), mid], [*u.start(), *u.end()]);
    let events = clip_surface_curve_to_face_with_operation(
        &spatial,
        &witness,
        &face,
        Tolerance::DEFAULT,
        &mut |a, b, t| a.intersection_events_with_curve(b, t),
    )
    .unwrap();
    let curves = events
        .iter()
        .filter_map(|e| {
            if let SurfaceSurfaceIntersectionEvent::Curve(c) = e {
                Some(c)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(curves.len(), 2);
    assert!(curves[0].domain().end() < curves[1].domain().start());
    for curve in curves {
        for t in [*curve.domain().start(), *curve.domain().end()] {
            let q = curve.evaluate(t).unwrap();
            assert!((q.x().hypot(q.y()) - 2.).abs() < 1e-9);
        }
    }
    let wrong = spatial
        .transformed(
            crate::AffineTransform3::try_new(
                [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                Vector3::try_new(0., 0., 0.1).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    assert!(
        clip_surface_curve_to_face_with_operation(
            &wrong,
            &witness,
            &face,
            Tolerance::DEFAULT,
            &mut |a, b, t| a.intersection_events_with_curve(b, t)
        )
        .is_err()
    );
}
