use super::*;
use crate::Frame3;
fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn uv(x: Real, y: Real) -> Point2 {
    Point2::try_new(x, y).unwrap()
}
fn tol() -> Tolerance {
    Tolerance::try_new(1e-6, 1e-12, 1e-9).unwrap()
}
fn frame() -> Frame3 {
    Frame3::try_from_x_and_normal(
        p(0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap()
}
#[test]
fn covering_space_loops_preserve_winding_reversal_and_tangent_seams() {
    let surface = NurbsSurface::try_cylinder(frame(), 3., 0., 6.).unwrap();
    for reverse in [false, true] {
        for laps in [1, 2] {
            let mut points = (0..4 * laps)
                .map(|i| {
                    uv(
                        surface
                            .parameter_at_u(0.1 + 0.25 * (i % 4) as Real)
                            .unwrap(),
                        surface.parameter_at_v(0.5).unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            if reverse {
                points.reverse();
            }
            let result = try_interpolate_curve_on_surface(&surface, &points, true, tol()).unwrap();
            assert!(result.curve.is_closed().unwrap());
            assert_eq!(result.seam_count, laps);
            let a = result
                .curve
                .evaluate_with_second_derivative(*result.curve.domain().start())
                .unwrap();
            let b = result
                .curve
                .evaluate_with_second_derivative(*result.curve.domain().end())
                .unwrap();
            assert!(a.0.distance_to(b.0).unwrap() < 1e-8);
            assert!(
                a.1.to_array()
                    .into_iter()
                    .zip(b.1.to_array())
                    .all(|(x, y)| (x - y).abs() < 1e-7)
            );
            for i in 0..=64 {
                let p = result
                    .curve
                    .evaluate(result.curve.parameter_at(i as Real / 64.).unwrap())
                    .unwrap();
                assert!((p.x().hypot(p.y()) - 3.).abs() < 5e-7);
                assert!((p.z() - 3.).abs() < 5e-7);
            }
        }
    }
}
#[test]
fn source_weights_and_translated_native_charts_keep_interpolation_locations() {
    let surface = NurbsSurface::try_bilinear([p(0., 0.), p(2., 0.), p(3., 4.), p(1., 4.)])
        .unwrap()
        .try_reparameterized(-2. ..=3., 1e6..=1e6 + 2.)
        .unwrap();
    let locations = [uv(-1., 1e6 + 0.4), uv(0.5, 1e6 + 1.6), uv(2., 1e6 + 0.6)];
    for gauge in [1e-100, 1e100] {
        let s = NurbsSurface::try_new_rational(
            surface.degree_u(),
            surface.degree_v(),
            surface.control_point_count_u(),
            surface.control_point_count_v(),
            surface
                .control_points()
                .iter()
                .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap())
                .collect(),
            surface.knots_u().to_vec(),
            surface.knots_v().to_vec(),
        )
        .unwrap();
        let before = s.clone();
        let result = try_interpolate_curve_on_surface(&s, &locations, false, tol()).unwrap();
        assert_eq!(s, before);
        assert!(result.maximum_interpolation_error < 1e-8);
        assert!(result.surface_image_bound < 5e-7);
    }
}
#[test]
fn overshoot_uses_a_private_support_and_invalid_points_fail_without_mutation() {
    let surface = NurbsSurface::try_bilinear([p(0., 0.), p(1., 0.), p(1., 1.), p(0., 1.)]).unwrap();
    let before = surface.clone();
    let result = try_interpolate_curve_on_surface(
        &surface,
        &[uv(0.1, 0.1), uv(0.5, 1.), uv(0.9, 0.2)],
        false,
        tol(),
    )
    .unwrap();
    let support = result.extended_support.as_ref().unwrap();
    assert!(*support.domain_v().end() > 1.);
    let maximum = (0..=256)
        .map(|i| {
            result
                .curve
                .evaluate(result.curve.parameter_at(i as Real / 256.).unwrap())
                .unwrap()
                .y()
        })
        .fold(0., Real::max);
    assert!(maximum > 1.0001);
    for points in [
        vec![uv(0.2, 0.3)],
        vec![uv(0.2, 0.3), uv(0.2, 0.3)],
        vec![uv(-0.1, 0.3), uv(0.8, 0.6)],
    ] {
        assert!(try_interpolate_curve_on_surface(&surface, &points, false, tol()).is_err());
    }
    assert_eq!(surface, before);
}
