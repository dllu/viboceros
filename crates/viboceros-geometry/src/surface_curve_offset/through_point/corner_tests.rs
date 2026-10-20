use super::*;
use crate::{Polyline3, WeightedPoint3};

fn p(x: Real, y: Real, scale: Real) -> Point3 {
    Point3::try_new(x * scale, y * scale, 0.).unwrap()
}
fn setup(scale: Real, closed: bool) -> (NurbsSurface, NurbsCurve, Tolerance) {
    let tolerance = Tolerance::try_new(scale * 1e-11, 1e-12, 1e-9).unwrap();
    let surface = NurbsSurface::try_bilinear([
        p(-10., -10., scale),
        p(10., -10., scale),
        p(10., 10., scale),
        p(-10., 10., scale),
    ])
    .unwrap();
    let points = if closed {
        vec![
            p(-2., -2., scale),
            p(2., -2., scale),
            p(2., 2., scale),
            p(-2., 2., scale),
            p(-2., -2., scale),
        ]
    } else {
        vec![p(-4., -2., scale), p(0., -2., scale), p(0., 4., scale)]
    };
    let source = Polyline3::try_new(points, tolerance)
        .unwrap()
        .to_native_nurbs()
        .unwrap()
        .try_reparameterized(3. ..=7.)
        .unwrap();
    let source = NurbsCurve::try_new_rational(
        source.degree(),
        source
            .control_points()
            .iter()
            .map(|c| WeightedPoint3::try_new(c.point(), -2_f64.powi(600)).unwrap())
            .collect(),
        source.knots().to_vec(),
    )
    .unwrap();
    (surface, source, tolerance)
}

#[test]
fn inner_corner_feet_share_one_signed_distance_across_scales_and_a_closed_seam() {
    for scale in [2_f64.powi(-100), 1., 2_f64.powi(100)] {
        for closed in [false, true] {
            let (surface, source, tolerance) = setup(scale, closed);
            let before = (surface.clone(), source.clone());
            for distance in [0.5, 2_f64.powi(-20)] {
                let point = if closed {
                    p(-2. + distance, -2. + distance, scale)
                } else {
                    p(-distance, -2. + distance, scale)
                };
                let result = surface_curve_offset_distance_through_point(
                    &surface, &source, point, tolerance,
                )
                .unwrap();
                assert!(
                    (result.signed_distance / scale - distance).abs() < 1e-10,
                    "{closed} {distance}: {result:?}"
                );
                assert!(result.point.distance_to(point).unwrap() < tolerance.absolute());
                let foot = source.evaluate(result.source_parameter).unwrap();
                assert!((foot.distance_to(point).unwrap() / scale - distance).abs() < 1e-10);
                assert!(source.domain().contains(&result.source_parameter));
                assert!(result.geodesic_queries < 8192);
            }
            assert_eq!((surface, source), before);
        }
    }
}

#[test]
fn outer_join_endpoints_keep_independent_left_and_right_normal_limits() {
    for scale in [2_f64.powi(-100), 1., 2_f64.powi(100)] {
        let (surface, source, tolerance) = setup(scale, false);
        for point in [p(0., -2.5, scale), p(0.5, -2., scale)] {
            let result =
                surface_curve_offset_distance_through_point(&surface, &source, point, tolerance)
                    .unwrap();
            assert!(
                (result.signed_distance / scale + 0.5).abs() < 1e-10,
                "{result:?}"
            );
            assert!((result.source_parameter - 5.).abs() < 1e-10, "{result:?}");
            assert!(result.point.distance_to(point).unwrap() < tolerance.absolute());
        }
    }
}

#[test]
fn an_outer_quadratic_bridge_point_is_outside_the_regular_normal_family() {
    let (surface, source, tolerance) = setup(1., false);
    assert!(
        surface_curve_offset_distance_through_point(
            &surface,
            &source,
            p(0.375, -2.375, 1.),
            tolerance
        )
        .is_err()
    );
}

#[test]
fn equal_magnitudes_on_opposite_signed_families_remain_ambiguous() {
    let (surface, _, tolerance) = setup(1., false);
    let source = Polyline3::try_new(
        vec![
            p(-4., 0., 1.),
            p(4., 0., 1.),
            p(4., 4., 1.),
            p(-4., 4., 1.),
            p(-4., 2., 1.),
            p(4., 2., 1.),
        ],
        tolerance,
    )
    .unwrap()
    .to_native_nurbs()
    .unwrap();
    assert!(matches!(
        surface_curve_offset_distance_through_point(&surface, &source, p(0., 1., 1.), tolerance),
        Err(GeometryError::AmbiguousCurveOffsetSide)
    ));
}

#[test]
fn nonuniform_rational_weights_recover_a_valid_foot_in_the_native_clock() {
    let (surface, _, tolerance) = setup(1., false);
    for gauge in [2_f64.powi(-400), -2_f64.powi(400)] {
        let source = NurbsCurve::try_new_rational(
            1,
            [((-4., -2.), 1.), ((0., -2.), 2.), ((0., 4.), 0.5)]
                .into_iter()
                .map(|((x, y), w)| WeightedPoint3::try_new(p(x, y, 1.), w * gauge).unwrap())
                .collect(),
            vec![3., 3., 5., 7., 7.],
        )
        .unwrap();
        assert!(source.is_rational());
        let result = surface_curve_offset_distance_through_point(
            &surface,
            &source,
            p(-0.5, -1.5, 1.),
            tolerance,
        )
        .unwrap();
        assert!((result.signed_distance - 0.5).abs() < 1e-10);
        // Weighted linear Bernstein fractions give t=41/9 or t=83/15.
        assert!(
            (result.source_parameter - 41. / 9.)
                .abs()
                .min((result.source_parameter - 83. / 15.).abs())
                < 1e-9,
            "{result:?}"
        );
        assert!(result.residual < tolerance.absolute());
    }
}
