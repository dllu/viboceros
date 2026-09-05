use super::*;
use crate::{Circle3, ParameterSide, PolyCurve3};
use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2};

fn point(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}
fn arc() -> CircularArc3 {
    CircularArc3::try_from_three_points(
        point(1.0, 0.0),
        point(FRAC_1_SQRT_2, FRAC_1_SQRT_2),
        point(0.0, 1.0),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn near(a: Point3, b: Point3) {
    assert!(a.distance_to(b).unwrap() < 2e-12, "{a:?} != {b:?}");
}
fn vector_near(a: Vector3, b: Vector3) {
    for (a, b) in a.to_array().into_iter().zip(b.to_array()) {
        assert!((a - b).abs() < 2e-12, "{a} != {b}");
    }
}

#[test]
fn native_arc_retains_angular_speed_not_rational_parameterization() {
    let segment = CurveSegment3::Arc(arc().try_reparameterized(-3.0..=5.0).unwrap());
    let native =
        PolyCurve3::try_with_segment_domains(vec![segment.clone()], vec![10.0, 14.0]).unwrap();
    for fraction in [0.0, 0.125, 0.375, 0.5, 0.875, 1.0] {
        let t = 10.0 + 4.0 * fraction;
        let (p, d, dd) = native
            .evaluate_with_second_derivative(t, ParameterSide::Right)
            .unwrap();
        let (s, c) = (fraction * FRAC_PI_2).sin_cos();
        let speed = FRAC_PI_2 / 4.0;
        near(p, point(c, s));
        vector_near(d, Vector3::try_new(-s * speed, c * speed, 0.0).unwrap());
        vector_near(
            dd,
            Vector3::try_new(-c * speed * speed, -s * speed * speed, 0.0).unwrap(),
        );
    }
    let rational = native.to_nurbs().unwrap();
    assert!(
        native
            .evaluate(10.5)
            .unwrap()
            .distance_to(rational.evaluate(10.5).unwrap())
            .unwrap()
            > 0.005
    );
    assert!(matches!(native.segments()[0], CurveSegment3::Arc(_)));
}

#[test]
fn native_arc_trim_reverse_and_similarity_preserve_parameterized_geometry() {
    let source = CurveSegment3::Arc(arc().try_reparameterized(-3.0..=5.0).unwrap());
    let trim = source.try_trimmed(-2.0..=4.0).unwrap();
    let reverse = trim.reversed().unwrap();
    assert_eq!(reverse.domain(), -4.0..=2.0);
    let transform = AffineTransform3::try_new(
        [[0.0, -2.0, 0.0], [2.0, 0.0, 0.0], [0.0, 0.0, 2.0]],
        Vector3::try_new(8.0, -6.0, 4.0).unwrap(),
    )
    .unwrap();
    let moved = trim.transformed(transform).unwrap();
    assert!(matches!(moved, CurveSegment3::Arc(_)));
    for i in 0..=64 {
        let t = -2.0 + 6.0 * i as Real / 64.0;
        near(source.evaluate(t).unwrap(), trim.evaluate(t).unwrap());
        near(trim.evaluate(t).unwrap(), reverse.evaluate(-t).unwrap());
        near(
            transform
                .transform_point(trim.evaluate(t).unwrap())
                .unwrap(),
            moved.evaluate(t).unwrap(),
        );
    }
}

#[test]
fn exact_nonuniform_transform_promotes_only_the_arc() {
    let a = arc();
    let source = PolyCurve3::try_new(vec![
        CurveSegment3::Line(
            LineSegment::try_new(point(2.0, 0.0), a.start().unwrap(), Tolerance::DEFAULT).unwrap(),
        ),
        CurveSegment3::Arc(a),
    ])
    .unwrap();
    let transform = AffineTransform3::try_new(
        [[1.0, 0.4, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 1.0]],
        Vector3::try_new(2.0, 3.0, 0.0).unwrap(),
    )
    .unwrap();
    let moved = source.transformed(transform).unwrap();
    assert!(matches!(moved.segments()[0], CurveSegment3::Line(_)));
    assert!(matches!(moved.segments()[1], CurveSegment3::NurbsCurve(_)));
    assert_eq!(moved.parameters(), source.parameters());
    let expected = source
        .try_deformable()
        .unwrap()
        .transformed(transform)
        .unwrap();
    assert_eq!(expected, moved);
    for (a, b) in source.segments().iter().zip(moved.segments()) {
        let rational = a.to_nurbs().unwrap();
        for i in 0..=32 {
            let t = a.parameter_at(i as Real / 32.0).unwrap();
            near(
                transform
                    .transform_point(rational.evaluate(t).unwrap())
                    .unwrap(),
                b.evaluate(t).unwrap(),
            );
        }
    }
}

#[test]
fn native_polyline_trim_keeps_nonuniform_parameters_and_piecewise_speed() {
    let curve = Polyline3::try_with_parameters(
        vec![point(0.0, 0.0), point(2.0, 0.0), point(2.0, 3.0)],
        vec![-5.0, -1.0, 8.0],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let segment = CurveSegment3::Polyline(curve);
    let trimmed = segment.try_trimmed(-3.0..=5.0).unwrap();
    let CurveSegment3::Polyline(line) = &trimmed else {
        panic!("polyline type lost")
    };
    assert_eq!(line.parameters(), &[-3.0, -1.0, 5.0]);
    assert_eq!(
        line.vertices(),
        &[point(1.0, 0.0), point(2.0, 0.0), point(2.0, 2.0)]
    );
    vector_near(
        trimmed.derivative_at(-2.0).unwrap(),
        Vector3::try_new(0.5, 0.0, 0.0).unwrap(),
    );
    vector_near(
        trimmed.derivative_at(0.0).unwrap(),
        Vector3::try_new(0.0, 1.0 / 3.0, 0.0).unwrap(),
    );
    assert_eq!(
        trimmed.spans().collect::<Vec<_>>(),
        vec![(-3.0, -1.0), (-1.0, 5.0)]
    );
    assert!(matches!(
        segment.try_trimmed(-4.0..=-2.0).unwrap(),
        CurveSegment3::Polyline(_)
    ));
}

#[test]
fn line_domains_survive_endpoint_edits_reverse_transform_and_nurbs_conversion() {
    let line = LineSegment::try_new(point(0.0, 0.0), point(4.0, 0.0), Tolerance::DEFAULT)
        .unwrap()
        .try_reparameterized(-3.0..=7.0)
        .unwrap();
    let edited = line
        .try_with_endpoints(None, Some(point(4.0, 2.0)), Tolerance::DEFAULT)
        .unwrap();
    let reverse = edited.reversed();
    assert_eq!(reverse.domain(), -7.0..=3.0);
    assert_eq!(edited.to_nurbs().unwrap().domain(), line.domain());
    let transform = AffineTransform3::from_translation(Vector3::try_new(8.0, 9.0, 0.0).unwrap());
    let transformed = edited.transformed(transform, Tolerance::DEFAULT).unwrap();
    assert_eq!(transformed.domain(), line.domain());
    for i in 0..=20 {
        let t = -3.0 + 10.0 * i as Real / 20.0;
        near(reverse.evaluate(-t).unwrap(), edited.evaluate(t).unwrap());
        near(
            transformed.evaluate(t).unwrap(),
            transform
                .transform_point(edited.evaluate(t).unwrap())
                .unwrap(),
        );
    }
}

#[test]
fn first_derivative_does_not_require_representable_second_derivative() {
    let curve = CurveSegment3::Arc(arc().try_reparameterized(0.0..=1e-200).unwrap());
    assert!(curve.evaluate_with_derivative(0.5e-200).is_ok());
    assert!(curve.evaluate_with_second_derivative(0.5e-200).is_err());
}

#[test]
fn arc_frame_operations_are_independent_of_large_world_translation() {
    let center = Point3::try_new(1e14, -2e14, 3e14).unwrap();
    let circle = Circle3::try_from_frame(
        center,
        1.0,
        Vector3::try_new(1.0, 0.0, 0.0)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let arc = CircularArc3::try_from_circle_sweep(circle, 1.0).unwrap();
    let reversed = arc.reversed(Tolerance::DEFAULT).unwrap();
    vector_near(
        reversed.x_axis().as_vector(),
        Vector3::try_new(1.0_f64.cos(), 1.0_f64.sin(), 0.0).unwrap(),
    );
    let trimmed = arc.try_trimmed(0.2..=0.8).unwrap();
    vector_near(
        trimmed.x_axis().as_vector(),
        Vector3::try_new(0.2_f64.cos(), 0.2_f64.sin(), 0.0).unwrap(),
    );
}

#[test]
fn invalid_native_domains_and_parameters_are_rejected() {
    let curve = CurveSegment3::Arc(arc());
    for domain in [0.0..=0.0, 2.0..=1.0, f64::NAN..=1.0, -f64::MAX..=f64::MAX] {
        assert!(curve.try_reparameterized(domain.clone()).is_err());
        assert!(curve.try_trimmed(domain).is_err());
    }
    for t in [f64::NAN, f64::INFINITY, -1.0, 10.0] {
        assert!(curve.evaluate(t).is_err());
    }
    assert!(curve.try_trimmed(0.0..=10.0).is_err());
}
