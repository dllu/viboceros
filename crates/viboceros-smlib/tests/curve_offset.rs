#![cfg(feature = "native")]
use viboceros_geometry::{
    Curve3, CurveOffsetCornerStyle, NurbsCurve, Point3, Tolerance, UnitVector3,
};
use viboceros_smlib::{offset_curve_parts, offset_curve_through_point};
fn p(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn compare(a: &Curve3, b: &Curve3) {
    let a = a.to_polycurve().unwrap();
    let b = b.to_polycurve().unwrap();
    assert_eq!(a.segments().len(), b.segments().len());
    for (a, b) in a.segments().iter().zip(b.segments()) {
        let a = a.to_nurbs().unwrap();
        let b = b.to_nurbs().unwrap();
        assert_eq!(a.degree(), b.degree());
        assert_eq!(a.knots(), b.knots());
        assert_eq!(a.control_points().len(), b.control_points().len());
        for (a, b) in a.control_points().iter().zip(b.control_points()) {
            assert!((a.weight() - b.weight()).abs() < 1e-10);
            assert!(a.point().distance_to(b.point()).unwrap() < 1e-8);
        }
        for i in 0..=32 {
            let t = a.parameter_at(i as f64 / 32.).unwrap();
            assert!(
                a.evaluate(t)
                    .unwrap()
                    .distance_to(b.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-8
            );
        }
    }
}
fn open() -> Curve3 {
    Curve3::NurbsCurve(
        NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(1., -0.5), p(2., 0.), p(2.5, 1.), p(2., 2.)],
            vec![0., 0., 0., 1., 1., 2., 2., 2.],
        )
        .unwrap(),
    )
}
fn closed() -> Curve3 {
    Curve3::NurbsCurve(
        NurbsCurve::try_new(
            2,
            vec![
                p(0., 0.),
                p(1., -0.5),
                p(2., 0.),
                p(2.5, 1.),
                p(2., 2.),
                p(1., 2.5),
                p(0., 2.),
                p(-0.5, 1.),
                p(0., 0.),
            ],
            vec![0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 4.],
        )
        .unwrap(),
    )
}
#[test]
fn native_offset_edits_preserve_open_and_closed_corner_nets_and_sources() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let normal = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
    let mut cases = 0;
    for source in [open(), closed()] {
        let before = source.clone();
        for distance in [-0.15, 0.15] {
            for corner in [
                CurveOffsetCornerStyle::None,
                CurveOffsetCornerStyle::Sharp,
                CurveOffsetCornerStyle::Round,
                CurveOffsetCornerStyle::Chamfer,
            ] {
                let a = offset_curve_parts(&source, distance, normal, tolerance, corner).unwrap();
                let b = source
                    .try_offset_parts(distance, normal, tolerance, corner)
                    .unwrap();
                assert_eq!(a.len(), b.len());
                for (a, b) in a.iter().zip(&b) {
                    compare(a, b);
                }
                cases += 1;
            }
        }
        assert_eq!(source, before);
    }
    assert_eq!(cases, 16);
}
#[test]
fn native_offset_through_point_reuses_corner_edits_and_rejects_invalid_input() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let normal = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
    let source = open();
    let before = source.clone();
    let parts = source
        .try_offset_parts(-0.15, normal, tolerance, CurveOffsetCornerStyle::None)
        .unwrap();
    let curve = parts[0].as_ref();
    let through = curve.evaluate(curve.parameter_at(0.3).unwrap()).unwrap();
    let (a, first) = offset_curve_through_point(
        &source,
        through,
        normal,
        tolerance,
        CurveOffsetCornerStyle::None,
    )
    .unwrap();
    let (b, second) = source
        .try_offset_through_point(through, normal, tolerance, CurveOffsetCornerStyle::None)
        .unwrap();
    assert!((a - b).abs() < 1e-10);
    assert_eq!(first.len(), second.len());
    for (a, b) in first.iter().zip(&second) {
        compare(a, b);
    }
    for distance in [0., f64::NAN, f64::INFINITY] {
        assert!(
            offset_curve_parts(
                &source,
                distance,
                normal,
                tolerance,
                CurveOffsetCornerStyle::Sharp
            )
            .is_err()
        );
    }
    assert_eq!(source, before);
}
