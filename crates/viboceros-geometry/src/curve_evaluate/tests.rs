use super::*;
use crate::{
    AffineTransform3, Circle3, CircularArc3, Curve3, CurveSegment3, Ellipse3, LineSegment,
    NurbsCurve, PointMorph, Polyline3, Tolerance, UnitVector3,
};
use std::f64::consts::{FRAC_PI_2, TAU};

pub(super) fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn axis(x: Real, y: Real, z: Real) -> UnitVector3 {
    Vector3::try_new(x, y, z)
        .unwrap()
        .normalized_nonzero()
        .unwrap()
}
fn circle() -> Circle3 {
    Circle3::try_new(
        p(1.0, 2.0, 3.0),
        3.0,
        axis(0.0, 0.0, 1.0),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
pub(super) fn ellipse() -> Ellipse3 {
    Ellipse3::try_new(
        p(1.0, 2.0, 3.0),
        5.0,
        2.0,
        axis(1.0, 0.0, 0.0),
        axis(0.0, 1.0, 0.0),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
pub(super) fn near(a: Point3, b: Point3) {
    assert!(a.distance_to(b).unwrap() < 2e-11, "{a:?} != {b:?}");
}
pub(super) fn near_vector(a: Vector3, b: Vector3) {
    for (a, b) in a.to_array().into_iter().zip(b.to_array()) {
        assert!((a - b).abs() < 2e-11, "{a} != {b}");
    }
}

#[test]
fn circular_domains_survive_reversal_scaling_and_conversion() {
    let source = circle();
    assert_eq!(source.domain(), 0.0..=TAU * 3.0);
    let source = source.try_reparameterized(-7.0..=13.0).unwrap();
    let reversed = source.reversed();
    assert_eq!(reversed.domain(), -13.0..=7.0);
    let transform = AffineTransform3::try_new(
        [[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0]],
        Vector3::try_new(2.0, 3.0, 4.0).unwrap(),
    )
    .unwrap();
    let moved = source
        .transformed_similarity(transform, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(moved.domain(), source.domain());
    assert_eq!(moved.to_nurbs().unwrap().domain(), source.domain());
    for i in 0..=128 {
        let t = CurveRef::Circle(&source)
            .parameter_at(i as Real / 128.0)
            .unwrap();
        near(source.evaluate(t).unwrap(), reversed.evaluate(-t).unwrap());
        near(
            moved.evaluate(t).unwrap(),
            transform
                .transform_point(source.evaluate(t).unwrap())
                .unwrap(),
        );
    }
    assert_eq!(
        source.evaluate(13.0).unwrap(),
        source.evaluate(-7.0).unwrap()
    );
}

#[test]
fn ellipse_native_jets_match_exact_rational_curve_across_all_quarters() {
    for source in [
        ellipse().try_reparameterized(-7.0..=13.0).unwrap(),
        ellipse().reversed(),
    ] {
        let rational = source.to_nurbs().unwrap();
        assert_eq!(source.domain(), rational.domain());
        for i in 0..=512 {
            let t = CurveRef::Ellipse(&source)
                .parameter_at(i as Real / 512.0)
                .unwrap();
            let (p, d, dd) = CurveRef::Ellipse(&source)
                .evaluate_with_second_derivative(t)
                .unwrap();
            let (q, e, ee) = rational.evaluate_with_second_derivative(t).unwrap();
            near(p, q);
            near_vector(d, e);
            near_vector(dd, ee);
        }
    }
    let source = ellipse();
    assert!(
        source
            .evaluate(FRAC_PI_2 * 0.125)
            .unwrap()
            .distance_to(source.point_at_angle(FRAC_PI_2 * 0.125).unwrap())
            .unwrap()
            > 0.005
    );
}

#[test]
fn leaf_sampling_and_single_segment_composites_share_native_parameters() {
    let sources = [
        Curve3::Circle(circle()),
        Curve3::Ellipse(ellipse()),
        Curve3::Arc(CircularArc3::try_from_circle_sweep(circle(), 1.3).unwrap()),
        Curve3::Line(
            LineSegment::try_new(p(1.0, 2.0, 3.0), p(4.0, 6.0, 15.0), Tolerance::DEFAULT).unwrap(),
        ),
        Curve3::Polyline(
            Polyline3::try_new(
                vec![p(0.0, 0.0, 0.0), p(3.0, 0.0, 0.0), p(3.0, 4.0, 0.0)],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        Curve3::NurbsCurve(
            NurbsCurve::try_clamped_uniform(
                2,
                vec![p(0.0, 0.0, 0.0), p(3.0, 4.0, 0.0), p(6.0, 1.0, 0.0)],
            )
            .unwrap(),
        ),
    ];
    for source in sources {
        let curve = source.try_reparameterized(-7.0..=13.0).unwrap();
        let composite = curve.to_polycurve().unwrap();
        let expected = curve
            .as_ref()
            .divide_by_count_samples(17, true, Tolerance::DEFAULT)
            .unwrap();
        let actual = CurveRef::PolyCurve(&composite)
            .divide_by_count_samples(17, true, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(expected.len(), 18);
        assert_eq!(actual.len(), 18);
        for (a, b) in expected.iter().zip(&actual) {
            assert!((a.parameter() - b.parameter()).abs() < 1e-9);
            near(a.point(), curve.as_ref().evaluate(a.parameter()).unwrap());
            near(a.point(), b.point());
            near_vector(a.tangent().as_vector(), b.tangent().as_vector());
        }
        assert_eq!(actual.first().unwrap().parameter(), -7.0);
        assert_eq!(actual.last().unwrap().parameter(), 13.0);
    }
}

#[test]
fn native_tangents_do_not_depend_on_derivative_scale_or_world_origin() {
    let circle = circle().try_reparameterized(0.0..=1e-310).unwrap();
    assert!(
        CurveRef::Circle(&circle)
            .evaluate_with_derivative(0.0)
            .is_err()
    );
    assert_eq!(
        CurveRef::Circle(&circle)
            .evaluate_with_tangent(0.0)
            .unwrap()
            .tangent(),
        axis(0.0, 1.0, 0.0)
    );
    let translated = Circle3::try_new(
        p(1e14, -2e14, 3e14),
        1.0,
        axis(0.0, 0.0, 1.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for i in 0..=32 {
        let angle = TAU * i as Real / 32.0;
        let sample = CurveRef::Circle(&translated)
            .evaluate_with_tangent(angle)
            .unwrap();
        near_vector(
            sample.tangent().as_vector(),
            Vector3::try_new(-angle.sin(), angle.cos(), 0.0).unwrap(),
        );
        near_vector(
            CurveRef::Circle(&translated)
                .curvature_vector(angle)
                .unwrap(),
            Vector3::try_new(-angle.cos(), -angle.sin(), 0.0).unwrap(),
        );
    }
}

#[test]
fn natural_circle_interval_overflow_does_not_reject_a_valid_partial_arc() {
    assert!(
        Circle3::try_new(
            p(0.0, 0.0, 0.0),
            4e307,
            axis(0.0, 0.0, 1.0),
            Tolerance::DEFAULT
        )
        .is_err()
    );
    let arc = CircularArc3::try_from_three_points(
        p(4e307, 0.0, 0.0),
        p(0.0, 4e307, 0.0),
        p(-4e307, 0.0, 0.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!(arc.length().unwrap().is_finite());
    assert!(arc.evaluate(*arc.domain().end()).is_ok());
}

#[test]
fn common_parameter_mapping_keeps_wide_nurbs_domain_support() {
    let curve = NurbsCurve::try_new(
        1,
        vec![p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0)],
        vec![-Real::MAX, -Real::MAX, Real::MAX, Real::MAX],
    )
    .unwrap();
    for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let expected = curve.parameter_at(fraction).unwrap();
        assert_eq!(
            CurveRef::NurbsCurve(&curve).parameter_at(fraction).unwrap(),
            expected
        );
        assert_eq!(
            CurveSegment3::NurbsCurve(curve.clone())
                .parameter_at(fraction)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn line_morph_preserves_the_native_interval() {
    struct Identity;
    impl PointMorph for Identity {
        fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
            Ok(point)
        }
    }
    let line = LineSegment::try_new(p(1.0, 2.0, 3.0), p(4.0, 6.0, 15.0), Tolerance::DEFAULT)
        .unwrap()
        .try_reparameterized(-7.0..=13.0)
        .unwrap();
    let morphed = Identity.morph_line(line).unwrap();
    assert_eq!(morphed.domain(), line.domain());
    for i in 0..=64 {
        let t = -7.0 + 20.0 * i as Real / 64.0;
        near(morphed.evaluate(t).unwrap(), line.evaluate(t).unwrap());
    }
}
