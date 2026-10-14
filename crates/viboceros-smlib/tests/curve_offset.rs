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
fn editing_operations() -> viboceros_geometry::CurveOffsetOperations {
    let mut operations = viboceros_smlib::curve_offset_operations();
    // Keep the original structural editing contract independent of fitting.
    operations.smooth = |_, _, _, _, _| Ok(None);
    operations.intersect = NurbsCurve::intersection_events_with_curve;
    operations
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
                let a = source
                    .try_offset_parts_with_operations(
                        distance,
                        normal,
                        tolerance,
                        corner,
                        Some(editing_operations()),
                    )
                    .unwrap();
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
    let (a, first) = source
        .try_offset_through_point_with_operations(
            through,
            normal,
            tolerance,
            CurveOffsetCornerStyle::None,
            Some(editing_operations()),
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

#[test]
fn native_fitted_offsets_preserve_corner_loci_closure_and_through_point_side() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let normal = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
    for source in [open(), closed()] {
        let before = source.clone();
        for distance in [-0.15, 0.15] {
            for corner in [
                CurveOffsetCornerStyle::None,
                CurveOffsetCornerStyle::Sharp,
                CurveOffsetCornerStyle::Round,
                CurveOffsetCornerStyle::Chamfer,
            ] {
                let native =
                    offset_curve_parts(&source, distance, normal, tolerance, corner).unwrap();
                let portable = source
                    .try_offset_parts(distance, normal, tolerance, corner)
                    .unwrap();
                assert_eq!(native.len(), portable.len());
                for (a, b) in native.iter().zip(&portable) {
                    assert_eq!(
                        a.as_ref().is_closed().unwrap(),
                        b.as_ref().is_closed().unwrap()
                    );
                    for (a, b) in [(a, b), (b, a)] {
                        for i in 0..=32 {
                            let point = a
                                .as_ref()
                                .evaluate(a.as_ref().parameter_at(i as f64 / 32.).unwrap())
                                .unwrap();
                            let parameter = b.as_ref().closest_parameter(point, tolerance).unwrap();
                            let error = point
                                .distance_to(b.as_ref().evaluate(parameter).unwrap())
                                .unwrap();
                            assert!(
                                error <= tolerance.absolute(),
                                "{corner:?}, {distance}: {error}"
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(source, before);
    }
    let source = open();
    let parts = source
        .try_offset_parts(-0.15, normal, tolerance, CurveOffsetCornerStyle::None)
        .unwrap();
    let through = parts[0]
        .as_ref()
        .evaluate(parts[0].as_ref().parameter_at(0.3).unwrap())
        .unwrap();
    let (distance, parts) = offset_curve_through_point(
        &source,
        through,
        normal,
        tolerance,
        CurveOffsetCornerStyle::None,
    )
    .unwrap();
    assert!((distance + 0.15).abs() <= tolerance.absolute());
    assert!(parts.iter().any(|part| {
        let parameter = part.as_ref().closest_parameter(through, tolerance).unwrap();
        part.as_ref()
            .evaluate(parameter)
            .unwrap()
            .distance_to(through)
            .unwrap()
            <= tolerance.absolute()
    }));
}

#[test]
fn native_single_kink_closed_source_keeps_fitted_seam_open_until_corner_policy() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let normal = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
    let curve = NurbsCurve::try_new(
        3,
        vec![p(0., 0.), p(4., 0.), p(0., 4.), p(0., 0.)],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let source = Curve3::NurbsCurve(curve.clone());
    let before = source.clone();
    let inward = offset_curve_parts(
        &source,
        0.2,
        normal,
        tolerance,
        CurveOffsetCornerStyle::None,
    )
    .unwrap();
    let [Curve3::NurbsCurve(inner)] = inward.as_slice() else {
        panic!("expected one trimmed loop")
    };
    assert!(inner.is_closed().unwrap());
    assert!(*inner.domain().start() > 0. && *inner.domain().end() < 1.);
    let outward = offset_curve_parts(
        &source,
        -0.2,
        normal,
        tolerance,
        CurveOffsetCornerStyle::None,
    )
    .unwrap();
    let [Curve3::NurbsCurve(outer)] = outward.as_slice() else {
        panic!("expected open seam")
    };
    assert!(!outer.is_closed().unwrap());
    assert!(
        outer
            .evaluate(0.)
            .unwrap()
            .distance_to(p(0., -0.2))
            .unwrap()
            <= tolerance.absolute()
    );
    assert!(
        outer
            .evaluate(1.)
            .unwrap()
            .distance_to(p(-0.2, 0.))
            .unwrap()
            <= tolerance.absolute()
    );
    for corner in [CurveOffsetCornerStyle::Round, CurveOffsetCornerStyle::Sharp] {
        let result = offset_curve_parts(&source, -0.2, normal, tolerance, corner).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].as_ref().is_closed().unwrap());
    }
    let relocated = Curve3::NurbsCurve(curve.try_split_at_parameters(&[0.5]).unwrap().remove(0));
    let result = offset_curve_parts(
        &relocated,
        0.2,
        normal,
        tolerance,
        CurveOffsetCornerStyle::None,
    )
    .unwrap();
    let [Curve3::NurbsCurve(inner)] = result.as_slice() else {
        panic!("expected relocated trimmed loop")
    };
    assert!(inner.is_closed().unwrap());
    assert!(*inner.domain().start() > 1. && *inner.domain().end() < 2.);
    assert_eq!(source, before);
}

#[test]
fn native_region_classification_reuses_bezier_and_split_with_independent_inside_checks() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use viboceros_geometry::{CurveOffsetSplit, GeometryError};
    static BEZIERS: AtomicUsize = AtomicUsize::new(0);
    static SPLITS: AtomicUsize = AtomicUsize::new(0);
    fn bezier(c: &NurbsCurve) -> Result<Vec<NurbsCurve>, GeometryError> {
        BEZIERS.fetch_add(1, Ordering::Relaxed);
        viboceros_smlib::curve_bezier_spans(c).map_err(|e| {
            GeometryError::InterpolationBackendFailed {
                message: e.to_string(),
            }
        })
    }
    fn split(c: &NurbsCurve, t: f64) -> Result<(NurbsCurve, NurbsCurve), GeometryError> {
        SPLITS.fetch_add(1, Ordering::Relaxed);
        viboceros_smlib::split_curve(c, t).map_err(|e| GeometryError::InterpolationBackendFailed {
            message: e.to_string(),
        })
    }
    let source = Curve3::NurbsCurve(
        NurbsCurve::try_new(
            3,
            vec![p(0., 0.), p(3., 0.), p(3., 3.), p(0., 0.)],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap(),
    );
    let before = source.clone();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let normal = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
    let mut operations = viboceros_smlib::curve_offset_operations();
    operations.bezier = bezier;
    operations.split = split as CurveOffsetSplit;
    let sign = source
        .offset_region_inward_sign_with_operations(normal, tolerance, Some(operations))
        .unwrap();
    assert_eq!(sign, Some(1.));
    assert_eq!(
        sign,
        source.offset_region_inward_sign(normal, tolerance).unwrap()
    );
    // C(t)=(9t(1-t),9t^2(1-t)). At X=1.5, its two boundary
    // Y values are about 0.317 and 1.183; X never exceeds 2.25.
    for (point, expected) in [(p(1.5, 0.75), true), (p(3., 0.75), false)] {
        let a = source
            .offset_region_contains_with_operations(point, normal, tolerance, Some(operations))
            .unwrap();
        assert_eq!(a, Some(expected));
        assert_eq!(
            a,
            source
                .offset_region_contains(point, normal, tolerance)
                .unwrap()
        );
    }
    assert!(BEZIERS.load(Ordering::Relaxed) > 0);
    assert!(SPLITS.load(Ordering::Relaxed) > 0);
    assert_eq!(source, before);
}

#[test]
fn native_region_queries_keep_gauges_boundary_errors_and_self_overlap_rejection() {
    use viboceros_geometry::{GeometryError, WeightedPoint3};
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let normal = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
    let source = closed();
    let Curve3::NurbsCurve(c) = &source else {
        unreachable!()
    };
    let operations = Some(viboceros_smlib::curve_offset_operations());
    for gauge in [1e-280, 1e280, -1e280] {
        let c = NurbsCurve::try_new_rational(
            2,
            c.control_points()
                .iter()
                .map(|p| WeightedPoint3::try_new(p.point(), gauge).unwrap())
                .collect(),
            c.knots().to_vec(),
        )
        .unwrap();
        let source = Curve3::NurbsCurve(c.clone());
        assert_eq!(
            source
                .offset_region_inward_sign_with_operations(normal, tolerance, operations)
                .unwrap(),
            source.offset_region_inward_sign(normal, tolerance).unwrap()
        );
        assert_eq!(
            source
                .offset_region_contains_with_operations(p(1., 1.), normal, tolerance, operations)
                .unwrap(),
            Some(true)
        );
        let boundary = c.evaluate(0.3).unwrap();
        assert!(matches!(
            source.offset_region_contains_with_operations(boundary, normal, tolerance, operations),
            Err(GeometryError::AmbiguousCurveOffsetSide)
        ));
    }
    let mut points = c
        .control_points()
        .iter()
        .map(|p| p.point())
        .collect::<Vec<_>>();
    points.extend(c.control_points()[1..].iter().map(|p| p.point()));
    let knots = [
        vec![0.; 3],
        (1..8).flat_map(|i| [i as f64; 2]).collect(),
        vec![8.; 3],
    ]
    .concat();
    let source = Curve3::NurbsCurve(NurbsCurve::try_new(2, points, knots).unwrap());
    let before = source.clone();
    assert!(matches!(
        source.offset_region_inward_sign_with_operations(normal, tolerance, operations),
        Err(GeometryError::SelfIntersectingOffsetRegion)
    ));
    assert!(matches!(
        source.offset_region_inward_sign(normal, tolerance),
        Err(GeometryError::SelfIntersectingOffsetRegion)
    ));
    assert_eq!(source, before);
}
