use super::*;
use crate::{LineSegment, Polyline3};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

struct Quartic;
impl PointMorph for Quartic {
    fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
        Point3::try_new(p.x(), p.y() + p.x().powi(4), p.z())
    }
}

#[test]
fn exhausted_refinement_returns_the_measured_error_instead_of_a_bad_curve() {
    let source = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0, 0.0), point(1.0, 0.0, 0.0)],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    assert!(matches!(fit(&Quartic, &source, Tolerance::DEFAULT, 5),
        Err(GeometryError::CurveMorphDidNotConverge { tolerance, deviation, maximum: 5 })
            if tolerance == Tolerance::DEFAULT.absolute() && deviation > tolerance));
}

#[test]
fn nonuniform_error_samples_detect_aliasing_of_the_old_uniform_grid() {
    struct Oscillation;
    impl PointMorph for Oscillation {
        fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
            Point3::try_new(p.x(), (std::f64::consts::PI * 96.0 * p.x()).sin(), 0.0)
        }
    }
    let source = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0, 0.0), point(1.0, 0.0, 0.0)],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    // The old thirds interpolation and sixteenths validation both see zero.
    assert!(matches!(fit(&Oscillation, &source, Tolerance::DEFAULT, 4),
        Err(GeometryError::CurveMorphDidNotConverge { deviation, .. }) if deviation > 0.5));
}

#[test]
fn constant_morph_at_a_large_world_offset_retains_exact_controls() {
    struct Constant;
    impl PointMorph for Constant {
        fn morph_point(&self, _: Point3) -> Result<Point3, GeometryError> {
            Ok(point(1e12, -2e12, 3e12))
        }
    }
    let line = LineSegment::try_new(
        point(0.0, 0.0, 0.0),
        point(1.0, 0.0, 0.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let curve = Constant.morph_line(line, Tolerance::DEFAULT).unwrap();
    for control in curve.control_points() {
        assert_eq!(control.point(), point(1e12, -2e12, 3e12));
    }
    for t in [0.0, 0.123, 0.5, 0.789, 1.0] {
        assert_eq!(curve.evaluate(t).unwrap(), point(1e12, -2e12, 3e12));
    }
}

#[test]
fn closed_polyline_morph_keeps_its_exact_seam_and_vertex_limits() {
    let tolerance = Tolerance::DEFAULT;
    let points = vec![
        point(0.0, 0.0, 0.0),
        point(1.0, 0.0, 0.0),
        point(1.0, 1.0, 0.0),
        point(0.0, 0.0, 0.0),
    ];
    let source =
        Polyline3::try_with_parameters(points, vec![2.0, 3.0, 7.0, 11.0], tolerance).unwrap();
    let curve = Quartic.morph_polyline(&source, tolerance).unwrap();
    assert_eq!(curve.evaluate(2.0).unwrap(), curve.evaluate(11.0).unwrap());
    for t in [2.0, 3.0, 7.0, 11.0] {
        let expected = Quartic.morph_point(source.evaluate(t).unwrap()).unwrap();
        for side in [ParameterSide::Left, ParameterSide::Right] {
            assert_eq!(curve.evaluate_on_side(t, side).unwrap(), expected);
        }
    }
}

#[test]
fn nonlinear_line_morph_meets_sampled_tolerance_in_its_native_domain() {
    let tolerance = Tolerance::DEFAULT;
    let line = LineSegment::try_new(point(0.0, 0.0, 0.0), point(1.0, 0.0, 0.0), tolerance)
        .unwrap()
        .try_reparameterized(-7.0..=13.0)
        .unwrap();
    let fitted = Quartic.morph_line(line, tolerance).unwrap();
    assert_eq!(fitted.domain(), line.domain());
    for i in 0..=2048 {
        let s = i as Real / 2048.0;
        let actual = fitted.evaluate(-7.0 + 20.0 * s).unwrap();
        assert!(actual.distance_to(point(s, s.powi(4), 0.0)).unwrap() <= tolerance.absolute());
    }
}

#[test]
fn nonlinear_polyline_morph_preserves_native_vertices_and_refines_each_segment() {
    let tolerance = Tolerance::DEFAULT;
    let source = Polyline3::try_with_parameters(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(1.0, 1.0, 0.0),
        ],
        vec![-7.0, -2.0, 13.0],
        tolerance,
    )
    .unwrap();
    let fitted = Quartic.morph_polyline(&source, tolerance).unwrap();
    assert_eq!(fitted.domain(), -7.0..=13.0);
    assert_eq!(fitted.evaluate(-2.0).unwrap(), point(1.0, 1.0, 0.0));
    for i in 0..=2048 {
        let t = -7.0 + 20.0 * i as Real / 2048.0;
        let expected = Quartic.morph_point(source.evaluate(t).unwrap()).unwrap();
        assert!(fitted.evaluate(t).unwrap().distance_to(expected).unwrap() <= tolerance.absolute());
    }
}

#[test]
fn identity_morph_keeps_full_order_positional_jumps_and_one_sided_values() {
    struct Identity;
    impl PointMorph for Identity {
        fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
            Ok(point)
        }
    }
    let source = NurbsCurve::try_new(
        1,
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(5.0, 3.0, 0.0),
            point(6.0, 3.0, 0.0),
        ],
        vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0],
    )
    .unwrap();
    let fitted = Identity
        .morph_nurbs_curve(&source, Tolerance::DEFAULT)
        .unwrap();
    for side in [crate::ParameterSide::Left, crate::ParameterSide::Right] {
        assert_eq!(
            fitted.evaluate_on_side(1.0, side).unwrap(),
            source.evaluate_on_side(1.0, side).unwrap()
        );
    }
    for t in [0.0, 0.25, 0.75, 1.25, 1.75, 2.0] {
        assert!(
            fitted
                .evaluate(t)
                .unwrap()
                .distance_to(source.evaluate(t).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}
