use super::*;
use viboceros_geometry::Vector3;

fn point(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}

fn composite() -> PolyCurve3 {
    PolyCurve3::try_with_segment_domains(
        vec![
            NurbsCurve::try_new(
                2,
                vec![point(0.0, 0.0), point(1.0, 2.0), point(3.0, 0.0)],
                vec![4.0, 4.0, 4.0, 6.0, 6.0, 6.0],
            )
            .unwrap(),
            NurbsCurve::try_new(
                1,
                vec![point(3.0, 0.0), point(4.0, 3.0)],
                vec![-1.0, -1.0, 1.0, 1.0],
            )
            .unwrap(),
        ],
        vec![-10.0, -3.0, 12.0],
    )
    .unwrap()
}

#[test]
fn affine_transform_and_nurbs_conversion_retain_composite_geometry() {
    let source = composite();
    let geometry = Geometry::PolyCurve(source.clone());
    let transform = AffineTransform3::try_new(
        [[2.0, 0.5, 0.0], [0.0, -3.0, 0.0], [0.0, 0.0, 1.0]],
        Vector3::try_new(7.0, 8.0, 9.0).unwrap(),
    )
    .unwrap();
    let transformed = geometry.transformed(transform, Tolerance::DEFAULT).unwrap();
    let Geometry::PolyCurve(result) = &transformed else {
        panic!("transform flattened polycurve")
    };
    assert_eq!(result.parameters(), source.parameters());
    assert_eq!(result.segments().len(), 2);
    assert_eq!(transformed.bounds(), result.control_point_bounds());
    let nurbs = geometry.nurbs_curve_representation().unwrap().unwrap();
    for i in 0..=100 {
        let t = source.parameter_at(i as f64 / 100.0).unwrap();
        let expected = source.evaluate(t).unwrap();
        assert!(nurbs.evaluate(t).unwrap().distance_to(expected).unwrap() < 1e-12);
        assert!(
            result
                .evaluate(t)
                .unwrap()
                .distance_to(transform.transform_point(expected).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn nonlinear_morph_keeps_segment_domains_and_connected_junctions() {
    struct Lift;
    impl PointMorph for Lift {
        fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
            Point3::try_new(point.x(), point.y(), point.x() * point.x())
        }
    }
    let source = composite();
    let Geometry::PolyCurve(result) = Geometry::PolyCurve(source.clone())
        .morphed(&Lift, Tolerance::DEFAULT)
        .unwrap()
    else {
        panic!("morph flattened polycurve")
    };
    assert_eq!(result.parameters(), source.parameters());
    for i in 0..=100 {
        let t = source.parameter_at(i as f64 / 100.0).unwrap();
        let expected = Lift.morph_point(source.evaluate(t).unwrap()).unwrap();
        assert!(result.evaluate(t).unwrap().distance_to(expected).unwrap() < 1e-8);
    }
}

#[test]
fn duplicate_detection_ignores_global_domain_and_whole_curve_direction() {
    let source = composite();
    let geometry = Geometry::PolyCurve(source.clone());
    assert!(
        geometry
            .geometrically_equals(&Geometry::PolyCurve(source.reversed().unwrap()))
            .unwrap()
    );
    assert!(
        geometry
            .geometrically_equals(&Geometry::PolyCurve(
                source.try_reparameterized(0.0..=1.0).unwrap()
            ))
            .unwrap()
    );
    let shifted = geometry
        .transformed(
            AffineTransform3::from_translation(Vector3::try_new(0.0, 0.0, 1.0).unwrap()),
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert!(!geometry.geometrically_equals(&shifted).unwrap());
    let mut segments = source.segments().to_vec();
    segments
        .push(NurbsCurve::try_clamped_uniform(1, vec![point(4.0, 3.0), point(0.0, 0.0)]).unwrap());
    let closed = PolyCurve3::try_new(segments).unwrap();
    assert!(
        !Geometry::PolyCurve(closed.clone())
            .geometrically_equals(&Geometry::PolyCurve(closed.reversed().unwrap()))
            .unwrap()
    );
    assert!(
        Geometry::PolyCurve(closed.clone())
            .geometrically_equals(&Geometry::PolyCurve(
                closed.try_reparameterized(0.0..=1.0).unwrap()
            ))
            .unwrap()
    );
}
