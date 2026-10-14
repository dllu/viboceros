use super::*;
use crate::{NurbsCurve2, Point2};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn surface() -> NurbsSurface {
    NurbsSurface::try_bilinear([p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.)])
        .unwrap()
}
#[test]
fn original_surface_certificates_reject_spatial_and_uv_errors() {
    let source = surface();
    for (height, uv_end, passed) in [(0., 1., true), (0.1, 1., false), (0., 0.9, false)] {
        let spatial = NurbsCurve::try_new(
            1,
            vec![p(0., 0.5, height), p(1., 0.5, height)],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let uv = NurbsCurve2::try_line(
            Point2::try_new(0., 0.5).unwrap(),
            Point2::try_new(uv_end, 0.5).unwrap(),
        )
        .unwrap();
        let reference = NurbsCurve2::try_line(
            Point2::try_new(0., 0.5).unwrap(),
            Point2::try_new(1., 0.5).unwrap(),
        )
        .unwrap();
        for wrong_first in [true, false] {
            let result = surface_surface_intersection_events_with_operation(
                &source,
                &source,
                Tolerance::DEFAULT,
                &mut |_, _, _| {
                    Ok::<_, GeometryError>(Some(vec![SurfaceSurfaceIntersectionProposal::Curve {
                        spatial: spatial.clone(),
                        first_uv: if wrong_first {
                            uv.clone()
                        } else {
                            reference.clone()
                        },
                        second_uv: if wrong_first {
                            reference.clone()
                        } else {
                            uv.clone()
                        },
                    }]))
                },
            );
            assert_eq!(result.is_ok(), passed);
        }
    }
}
#[test]
fn touching_point_policy_excludes_a_coplanar_corner_and_rejects_off_surface_points() {
    let a = surface();
    assert!(
        surface_surface_intersection_events_with_operation(
            &a,
            &a,
            Tolerance::DEFAULT,
            &mut |_, _, _| Ok::<_, GeometryError>(Some(vec![
                SurfaceSurfaceIntersectionProposal::Point(p(1., 1., 0.))
            ]))
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        surface_surface_intersection_events_with_operation(
            &a,
            &a,
            Tolerance::DEFAULT,
            &mut |_, _, _| Ok::<_, GeometryError>(Some(vec![
                SurfaceSurfaceIntersectionProposal::Point(p(0.5, 0.5, 1.))
            ]))
        )
        .is_err()
    );
}

#[test]
fn absent_producer_uses_portable_results_and_producer_errors_propagate() {
    let a = surface();
    let b = NurbsSurface::try_bilinear([
        p(0.5, 0., -1.),
        p(0.5, 1., -1.),
        p(0.5, 1., 1.),
        p(0.5, 0., 1.),
    ])
    .unwrap();
    assert_eq!(
        surface_surface_intersection_events_with_operation(
            &a,
            &b,
            Tolerance::DEFAULT,
            &mut |_, _, _| Ok::<_, GeometryError>(None),
        )
        .unwrap(),
        surface_surface_intersection_events(&a, &b, Tolerance::DEFAULT).unwrap()
    );
    assert!(matches!(
        surface_surface_intersection_events_with_operation(
            &a,
            &b,
            Tolerance::DEFAULT,
            &mut |_, _, _| Err::<Option<Vec<SurfaceSurfaceIntersectionProposal>>, _>(
                GeometryError::CurveIntersectionDidNotConverge
            ),
        ),
        Err(GeometryError::CurveIntersectionDidNotConverge)
    ));
}
