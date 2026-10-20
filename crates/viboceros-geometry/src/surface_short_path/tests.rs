use super::*;
use crate::{Frame3, WeightedPoint3};
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn uv(a: Real, b: Real) -> Point2 {
    Point2::try_new(a, b).unwrap()
}
fn tol() -> Tolerance {
    Tolerance::try_new(1e-6, 1e-12, 1e-9).unwrap()
}
#[test]
fn affine_paths_are_lines_across_units_gauges_and_native_domains() {
    for scale in [1e-6, 1., 1e6] {
        let s = NurbsSurface::try_bilinear([
            p(0., 0., 0.),
            p(2. * scale, 0., 0.),
            p(3. * scale, 4. * scale, scale),
            p(scale, 4. * scale, scale),
        ])
        .unwrap();
        let s = s.try_reparameterized(-2. ..=3., 1e6..=1e6 + 2.).unwrap();
        for gauge in [1e-20, 1e20] {
            let controls = s
                .control_points()
                .iter()
                .map(|c| WeightedPoint3::try_new(c.point(), c.weight() * gauge).unwrap())
                .collect();
            let s = NurbsSurface::try_new_rational(
                s.degree_u(),
                s.degree_v(),
                s.control_point_count_u(),
                s.control_point_count_v(),
                controls,
                s.knots_u().to_vec(),
                s.knots_v().to_vec(),
            )
            .unwrap();
            let a = uv(-1., 1e6 + 0.5);
            let b = uv(2., 1e6 + 1.5);
            let path =
                try_short_path_on_surface(&s, a, b, scaled_tolerance(tol(), scale).unwrap(), None)
                    .unwrap();
            assert_eq!(path.geodesic_queries, 0);
            let endpoints = [
                s.evaluate(a.x(), a.y()).unwrap(),
                s.evaluate(b.x(), b.y()).unwrap(),
            ];
            assert!(
                (path.distance - endpoints[0].distance_to(endpoints[1]).unwrap()).abs()
                    < scale * 1e-8
            );
            assert!(path.surface_image_bound < scale * 5e-7);
        }
    }
}
#[test]
fn cylindrical_seam_paths_choose_the_short_unrolled_route() {
    let s = NurbsSurface::try_cylinder(
        Frame3::try_from_x_and_normal(
            p(0., 0., 0.),
            Vector3::try_new(1., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap(),
        3.,
        0.,
        6.,
    )
    .unwrap();
    let a = uv(
        s.parameter_at_u(0.95).unwrap(),
        s.parameter_at_v(0.2).unwrap(),
    );
    let b = uv(
        s.parameter_at_u(0.05).unwrap(),
        s.parameter_at_v(0.8).unwrap(),
    );
    let result = try_short_path_on_surface(&s, a, b, tol(), None).unwrap();
    let from = s.evaluate(a.x(), a.y()).unwrap();
    let to = s.evaluate(b.x(), b.y()).unwrap();
    let sweep = to.y().atan2(to.x()) - from.y().atan2(from.x());
    assert!((result.distance - (3. * sweep).hypot(to.z() - from.z())).abs() < 1e-10);
    assert!(result.distance < 5.);
    for i in 0..=64 {
        let t = i as Real / 64.;
        let q = result
            .curve
            .evaluate(result.curve.parameter_at(t).unwrap())
            .unwrap();
        assert!((q.x().hypot(q.y()) - 3.).abs() < 2e-8);
        assert!((q.z() - ((1. - t) * from.z() + t * to.z())).abs() < 1e-10);
    }
}
#[test]
fn general_paths_reverse_and_remain_stationary_on_a_regular_patch() {
    let s = NurbsSurface::try_bilinear([
        p(-5., -5., 0.),
        p(5., -5., 0.),
        p(5., 5., 3.),
        p(-5., 5., -1.),
    ])
    .unwrap();
    let a = uv(0.15, 0.2);
    let b = uv(0.85, 0.8);
    let forward = try_short_path_on_surface(&s, a, b, tol(), None).unwrap();
    let reverse = try_short_path_on_surface(&s, b, a, tol(), None).unwrap();
    assert!((forward.distance - reverse.distance).abs() < 1e-7);
    assert!(forward.geodesic_queries > 0 && forward.geodesic_queries <= MAX_QUERIES);
    for i in 1..16 {
        let t = i as Real / 16.;
        let p = forward
            .curve
            .evaluate(forward.curve.parameter_at(t).unwrap())
            .unwrap();
        let q = reverse
            .curve
            .evaluate(reverse.curve.parameter_at(1. - t).unwrap())
            .unwrap();
        assert!(p.distance_to(q).unwrap() < 1e-7);
        let (u, v) = s.closest_parameters(p, tol()).unwrap();
        let tangent = forward
            .curve
            .derivative_at(forward.curve.parameter_at(t).unwrap())
            .unwrap();
        let acceleration = forward
            .curve
            .evaluate_with_second_derivative(forward.curve.parameter_at(t).unwrap())
            .unwrap()
            .2;
        let n = s.normal_at(u, v).unwrap().as_vector();
        assert!(tangent.dot(n).unwrap().abs() < 1e-5);
        // A geodesic's ambient acceleration is normal to the surface.
        let tangential = sum(acceleration, 1., n, -acceleration.dot(n).unwrap()).unwrap();
        assert!(tangential.length().unwrap() < 1e-3);
    }
}

#[test]
fn sphere_pole_longitudes_are_redundant_and_outside_parameters_fail() {
    let frame = Frame3::try_from_x_and_normal(
        p(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap();
    let s = NurbsSurface::try_sphere(frame, 4.).unwrap();
    let a = uv(
        s.parameter_at_u(0.1).unwrap(),
        s.parameter_at_v(0.5).unwrap(),
    );
    for longitude in [0., 0.3, 1.] {
        let b = uv(s.parameter_at_u(longitude).unwrap(), *s.domain_v().end());
        let path = try_short_path_on_surface(&s, a, b, tol(), None).unwrap();
        assert!((path.distance - 2. * PI).abs() < 1e-12);
        for i in 0..=64 {
            let q = path
                .curve
                .evaluate(path.curve.parameter_at(i as Real / 64.).unwrap())
                .unwrap();
            assert!((q.vector_to(p(0., 0., 0.)).unwrap().length().unwrap() - 4.).abs() < 1e-12);
        }
    }
    assert!(try_short_path_on_surface(&s, uv(-1., 0.), a, tol(), None).is_err());
}
