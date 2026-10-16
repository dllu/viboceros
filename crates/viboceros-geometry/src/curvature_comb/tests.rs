use super::*;
use crate::{Circle3, CurveRef, Tolerance};
fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}

#[test]
fn curvature_comb_circle_has_analytic_curvature_and_two_sides_of_each_span() {
    let c = Circle3::try_from_center_point(
        p(0., 0.),
        p(2., 0.),
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let c = c.to_nurbs().unwrap();
    let r = curvature_comb(&c, 16, 100).unwrap();
    assert_eq!(r.strips.len(), 4);
    assert_eq!(r.hairs.len(), 68);
    for h in r.hairs {
        assert!((h.point.distance_to(p(0., 0.)).unwrap() - 2.).abs() < 1e-13);
        assert!((h.curvature.length().unwrap() - 0.5).abs() < 1e-13);
        assert!(
            h.point
                .translated(h.curvature.scaled(4.).unwrap())
                .unwrap()
                .distance_to(p(0., 0.))
                .unwrap()
                < 1e-13
        );
    }
}
#[test]
fn curvature_comb_preserves_curvature_jumps_at_tangent_continuous_knots() {
    let c = NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(1., 1.), p(2., 0.), p(3., -1.), p(4., 3.)],
        vec![0., 0., 0., 1., 1., 2., 2., 2.],
    )
    .unwrap();
    let r = curvature_comb(&c, 8, 30).unwrap();
    assert_eq!(r.strips, vec![0..9, 9..18]);
    let a = r.hairs[8];
    let b = r.hairs[9];
    assert_eq!(a.parameter, 1.);
    assert_eq!(b.parameter, 1.);
    assert_eq!(a.point, b.point);
    assert!(a.curvature.dot(b.curvature).unwrap() < 0.);
    for (h, side) in [(a, ParameterSide::Left), (b, ParameterSide::Right)] {
        assert_eq!(
            h.curvature,
            CurveRef::NurbsCurve(&c)
                .curvature_vector_on_side(1., side)
                .unwrap()
        );
    }
}
#[test]
fn curvature_comb_singular_samples_break_strips_and_budget_is_strict() {
    let c = NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(0., 0.), p(2., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let r = curvature_comb(&c, 8, 9).unwrap();
    assert_eq!(r.singular_samples, 1);
    assert_eq!(r.hairs.len(), 8);
    assert!(r.hairs.iter().all(|h| h.curvature.length().unwrap() == 0.));
    assert_eq!(
        curvature_comb(&c, 8, 8).unwrap_err(),
        GeometryError::CurvatureCombLimit
    );
    assert_eq!(
        curvature_comb(&c, 0, 100).unwrap_err(),
        GeometryError::CurvatureCombLimit
    );
    assert_eq!(
        curvature_comb(&c, usize::MAX, usize::MAX).unwrap_err(),
        GeometryError::CurvatureCombLimit
    );
}
