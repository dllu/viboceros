use super::*;
use crate::{Circle3, CurveRef, Tolerance};
mod adaptive;
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
#[test]
fn curvature_envelopes_join_smooth_circle_spans_and_close_the_source_seam() {
    let c = Circle3::try_from_center_point(
        p(0., 0.),
        p(2., 0.),
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let comb = curvature_comb(&c, 16, 100).unwrap();
    let curves = comb.envelope_polylines(1., Tolerance::DEFAULT).unwrap();
    assert_eq!(curves.len(), 1);
    let poly = &curves[0];
    assert_eq!(poly.vertices().len(), 65);
    assert_eq!(poly.vertices().first(), poly.vertices().last());
    for p in poly.vertices() {
        assert!((p.distance_to(Point3::try_new(0., 0., 0.).unwrap()).unwrap() - 2.5).abs() < 1e-13);
    }
}
#[test]
fn curvature_envelopes_separate_kinks_and_g1_jumps_but_join_c2_spans() {
    for (degree, points, knots, expected) in [
        (
            2,
            vec![p(0., 0.), p(1., 1.), p(2., 0.), p(3., -1.), p(4., 3.)],
            vec![0., 0., 0., 1., 1., 2., 2., 2.],
            2,
        ),
        (
            2,
            vec![p(0., 0.), p(1., 1.), p(2., 0.), p(3., 1.), p(4., 0.)],
            vec![0., 0., 0., 1., 1., 2., 2., 2.],
            2,
        ),
        (
            3,
            vec![p(0., 0.), p(1., 2.), p(2., 2.), p(3., 1.), p(4., 0.)],
            vec![0., 0., 0., 0., 1., 2., 2., 2., 2.],
            1,
        ),
    ] {
        let c = NurbsCurve::try_new(degree, points, knots).unwrap();
        let comb = curvature_comb(&c, 16, 100).unwrap();
        assert_eq!(
            comb.envelope_polylines(1., Tolerance::DEFAULT)
                .unwrap()
                .len(),
            expected
        );
    }
}
#[test]
fn curvature_envelopes_omit_flat_strips_and_reject_invalid_scales_or_indices() {
    let c = NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(0., 0.), p(2., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let mut comb = curvature_comb(&c, 8, 9).unwrap();
    assert!(
        comb.envelope_polylines(1., Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
    for factor in [-1., f64::NAN, f64::INFINITY] {
        assert!(comb.envelope_polylines(factor, Tolerance::DEFAULT).is_err());
    }
    comb.strips.clear();
    comb.strips.push(0..100);
    assert!(comb.envelope_polylines(1., Tolerance::DEFAULT).is_err());
}
#[test]
fn curvature_envelopes_join_across_a_closed_seam_without_closing_discontinuous_pieces() {
    let circle = Circle3::try_from_center_point(
        p(0., 0.),
        p(2., 0.),
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let mut controls = circle.control_points().to_vec();
    controls[3] = crate::WeightedPoint3::try_new(p(-3., 2.), controls[3].weight()).unwrap();
    let c =
        NurbsCurve::try_new_rational(circle.degree(), controls, circle.knots().to_vec()).unwrap();
    assert!(c.is_closed().unwrap());
    let comb = curvature_comb(&c, 16, 100).unwrap();
    let outlines = comb.envelope_polylines(1., Tolerance::DEFAULT).unwrap();
    assert_eq!(outlines.len(), 2);
    assert_eq!(
        outlines
            .iter()
            .map(|p| p.vertices().len())
            .collect::<Vec<_>>(),
        vec![17, 49]
    );
    assert!(
        outlines
            .iter()
            .all(|p| p.vertices().first() != p.vertices().last())
    );
}
