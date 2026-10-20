use super::*;
use crate::Frame3;
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn unit(x: Real, y: Real, z: Real) -> UnitVector3 {
    v([x, y, z]).unwrap().normalized_nonzero().unwrap()
}
fn frame() -> Frame3 {
    Frame3::try_from_unit_normal(p(0., 0., 0.), unit(0., 0., 1.)).unwrap()
}
fn tol(scale: Real) -> Tolerance {
    Tolerance::try_new(1e-8 * scale, 1e-12, 1e-9).unwrap()
}

#[test]
fn skew_plane_shots_are_invariant_under_world_and_parameter_scaling() {
    for exponent in [-200, 0, 200] {
        let scale = 2_f64.powi(exponent);
        let pose =
            Frame3::try_from_unit_normal(p(5. * scale, -7. * scale, 11. * scale), unit(2., 3., 6.))
                .unwrap();
        let original = NurbsSurface::try_bilinear(
            [[0., 0., 0.], [2., 0., 0.], [3., 4., 0.], [1., 4., 0.]]
                .map(|q| pose.point_at(q.map(|x| x * scale)).unwrap()),
        )
        .unwrap();
        for power in [-600, 0, 600] {
            let domain_scale = 2_f64.powi(power);
            let s = original
                .try_reparameterized(
                    -domain_scale..=domain_scale,
                    2. * domain_scale..=5. * domain_scale,
                )
                .unwrap();
            let start = Point2::try_new(
                s.parameter_at_u(0.3).unwrap(),
                s.parameter_at_v(0.4).unwrap(),
            )
            .unwrap();
            let source = s.evaluate(start.x(), start.y()).unwrap();
            let direction = combine(
                pose.x_axis().as_vector(),
                0.6,
                pose.y_axis().as_vector(),
                0.8,
            )
            .unwrap()
            .normalized_nonzero()
            .unwrap();
            let before = s.clone();
            let prepared = PreparedSurfaceGeodesic::new(&s).unwrap();
            for distance in [0., 0.3 * scale, -0.3 * scale] {
                let result = prepared
                    .point(start, direction, distance, tol(scale))
                    .unwrap();
                let a = source.to_array();
                let b = direction.as_vector().to_array();
                let expected =
                    Point3::try_from(std::array::from_fn(|i| a[i] + distance * b[i])).unwrap();
                assert!(result.point.distance_to(expected).unwrap() / scale < 1e-12);
                assert!(result.estimated_error <= tol(scale).absolute());
                assert!(result.maximum_speed_error <= tol(scale).absolute());
            }
            assert_eq!(s, before);
        }
    }
}

#[test]
fn cylinder_shots_match_unrolled_straight_lines_and_cross_the_seam() {
    let s = NurbsSurface::try_cylinder(frame(), 3., -4., 4.).unwrap();
    let start = Point2::try_new(6.1, 0.5).unwrap();
    let source = s.evaluate(start.x(), start.y()).unwrap();
    let theta = source.y().atan2(source.x());
    let direction = unit(-0.6 * theta.sin(), 0.6 * theta.cos(), 0.8);
    let prepared = PreparedSurfaceGeodesic::new(&s).unwrap();
    for distance in [-2., 2., 7.] {
        if distance == 7. {
            assert!(prepared.point(start, direction, distance, tol(1.)).is_err());
            continue;
        }
        let result = prepared.point(start, direction, distance, tol(1.)).unwrap();
        let angle = theta + 0.6 * distance / 3.;
        let expected = p(
            3. * angle.cos(),
            3. * angle.sin(),
            source.z() + 0.8 * distance,
        );
        assert!(
            result.point.distance_to(expected).unwrap() < 2e-9,
            "{result:?} {expected:?}"
        );
        let expected_tangent = unit(-0.6 * angle.sin(), 0.6 * angle.cos(), 0.8);
        let expected_tangent = if distance < 0. {
            expected_tangent.opposite()
        } else {
            expected_tangent
        };
        assert!(
            result
                .tangent
                .as_vector()
                .angle_to(expected_tangent.as_vector())
                .unwrap()
                < 1e-9
        );
    }
}

#[test]
fn oblique_sphere_shots_follow_great_circles_and_reverse() {
    let s = NurbsSurface::try_sphere(frame(), 4.).unwrap();
    let start = Point2::try_new(1., 0.25).unwrap();
    let (point, du, dv) = s.evaluate_with_derivatives(start.x(), start.y()).unwrap();
    let direction = combine(
        du.normalized_nonzero().unwrap().as_vector(),
        0.6,
        dv.normalized_nonzero().unwrap().as_vector(),
        0.8,
    )
    .unwrap()
    .normalized_nonzero()
    .unwrap();
    let prepared = PreparedSurfaceGeodesic::new(&s).unwrap();
    for distance in [-1.2, 1.2] {
        let result = prepared.point(start, direction, distance, tol(1.)).unwrap();
        let a = point.to_array();
        let b = direction.as_vector().to_array();
        let angle = distance / 4.;
        let expected = Point3::try_from(std::array::from_fn(|i| {
            a[i] * angle.cos() + 4. * b[i] * angle.sin()
        }))
        .unwrap();
        assert!(
            result.point.distance_to(expected).unwrap() < 3e-9,
            "{result:?} {expected:?}"
        );
        let backwards = prepared
            .point(result.uv, result.tangent, -distance.abs(), tol(1.))
            .unwrap();
        assert!(backwards.point.distance_to(point).unwrap() < 6e-9);
    }
}

#[test]
fn boundary_normal_direction_poles_and_creased_charts_fail_explicitly() {
    let s =
        NurbsSurface::try_bilinear([p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.)])
            .unwrap();
    let prepared = PreparedSurfaceGeodesic::new(&s).unwrap();
    let start = Point2::try_new(0.5, 0.5).unwrap();
    assert!(
        prepared
            .point(start, unit(1., 0., 0.), 1., tol(1.))
            .is_err()
    );
    let endpoint = prepared
        .point(start, unit(1., 0., 0.), 0.5, tol(1.))
        .unwrap();
    assert!(endpoint.point.distance_to(p(1., 0.5, 0.)).unwrap() < 1e-12);
    assert!(
        prepared
            .point(
                start,
                unit(1., 0., 0.),
                0.5 + 4. * Real::EPSILON,
                Tolerance::try_new(1e-18, 1e-14, 1e-12).unwrap()
            )
            .is_err()
    );
    assert!(
        prepared
            .point(start, unit(0., 0., 1.), 0.1, tol(1.))
            .is_err()
    );
    assert!(
        prepared
            .point(start, unit(1., 0., 0.), Real::NAN, tol(1.))
            .is_err()
    );
    let sphere = NurbsSurface::try_sphere(frame(), 4.).unwrap();
    assert!(
        PreparedSurfaceGeodesic::new(&sphere)
            .unwrap()
            .point(
                Point2::try_new(0., *sphere.domain_v().end()).unwrap(),
                unit(1., 0., 0.),
                1.,
                tol(1.)
            )
            .is_err()
    );
    let crease = NurbsSurface::try_new(
        1,
        1,
        3,
        2,
        vec![
            p(0., 0., 0.),
            p(0.5, 0., 0.),
            p(1., 0., 1.),
            p(0., 1., 0.),
            p(0.5, 1., 0.),
            p(1., 1., 1.),
        ],
        vec![0., 0., 0.5, 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let error = PreparedSurfaceGeodesic::new(&crease)
        .unwrap()
        .point(
            Point2::try_new(0.25, 0.5).unwrap(),
            unit(1., 0., 0.),
            0.5,
            tol(1.),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        GeometryError::InvalidSurfaceGeodesic {
            context: "non-C1 surface chart transition"
        }
    ));
}

#[test]
fn warped_patch_matches_an_independent_implicit_world_space_equation() {
    // F=z-.04xy-.2x-.1y-.5. An embedded geodesic obeys
    // p''=-(v^T Hess(F) v)/|grad(F)|² grad(F). This independent Cartesian
    // equation uses neither NURBS jets nor the UV metric/Christoffel solve.
    let s = NurbsSurface::try_bilinear([
        p(-5., -5., 0.),
        p(5., -5., 0.),
        p(5., 5., 3.),
        p(-5., 5., -1.),
    ])
    .unwrap();
    let start = Point2::try_new(0.3, 0.4).unwrap();
    let (point, du, _) = s.evaluate_with_derivatives(start.x(), start.y()).unwrap();
    let direction = s
        .normal_at(start.x(), start.y())
        .unwrap()
        .as_vector()
        .cross(du)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let rhs = |y: [Real; 6]| {
        let n = [-0.04 * y[1] - 0.2, -0.04 * y[0] - 0.1, 1.];
        let factor = 0.08 * y[3] * y[4] / n.iter().map(|x| x * x).sum::<Real>();
        [
            y[3],
            y[4],
            y[5],
            factor * n[0],
            factor * n[1],
            factor * n[2],
        ]
    };
    let add = |a: [Real; 6], b: [Real; 6], h: Real| std::array::from_fn(|i| a[i] + h * b[i]);
    for distance in [-2., 2.] {
        let q = point.to_array();
        let t = direction.as_vector().to_array();
        let mut state = [q[0], q[1], q[2], t[0], t[1], t[2]];
        let h = distance / 2048.;
        for _ in 0..2048 {
            let a = rhs(state);
            let b = rhs(add(state, a, h * 0.5));
            let c = rhs(add(state, b, h * 0.5));
            let d = rhs(add(state, c, h));
            state =
                std::array::from_fn(|i| state[i] + h * (a[i] + 2. * b[i] + 2. * c[i] + d[i]) / 6.);
        }
        let expected = p(state[0], state[1], state[2]);
        let result = PreparedSurfaceGeodesic::new(&s)
            .unwrap()
            .point(start, direction, distance, tol(1.))
            .unwrap();
        assert!(
            result.point.distance_to(expected).unwrap() < 1e-9,
            "{result:?} {expected:?}"
        );
        assert!(
            (state[2] - 0.04 * state[0] * state[1] - 0.2 * state[0] - 0.1 * state[1] - 0.5).abs()
                < 1e-12
        );
    }
}

#[test]
fn endpoints_must_be_representable_in_a_translated_native_chart() {
    let s =
        NurbsSurface::try_bilinear([p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.)])
            .unwrap()
            .try_reparameterized(32.0..=32. + 2_f64.powi(-40), 0.0..=1.)
            .unwrap();
    let start = Point2::try_new(s.parameter_at_u(0.5).unwrap(), 0.5).unwrap();
    assert!(matches!(
        PreparedSurfaceGeodesic::new(&s)
            .unwrap()
            .point(start, unit(1., 0., 0.), 0.123, tol(1.)),
        Err(GeometryError::InvalidSurfaceGeodesic {
            context: "native parameter chart cannot represent the endpoint accurately"
        })
    ));
}

#[test]
fn world_coordinate_roundoff_cannot_silently_consume_requested_travel() {
    let origin = 2_f64.powi(350);
    let width = 2_f64.powi(310);
    let s = NurbsSurface::try_bilinear([
        p(origin, 0., 0.),
        p(origin + width, 0., 0.),
        p(origin + width, width, 0.),
        p(origin, width, 0.),
    ])
    .unwrap();
    let prepared = PreparedSurfaceGeodesic::new(&s).unwrap();
    let start = Point2::try_new(0.5, 0.5).unwrap();
    assert!(
        prepared
            .point(start, unit(1., 0., 0.), 1., tol(1.))
            .is_err()
    );
    assert!(matches!(
        prepared.point(start, unit(1., 0., 0.), 0.123 * width, tol(width)),
        Err(GeometryError::InvalidSurfaceGeodesic {
            context: "world coordinates cannot represent the endpoint accurately"
        })
    ));
}

#[test]
fn thin_skew_chart_does_not_square_the_metric_condition_number() {
    for power in [-40, -80, -200] {
        let width = 2_f64.powi(power);
        let s = NurbsSurface::try_bilinear([
            p(0., 0., 0.),
            p(1., 0., 0.),
            p(2., width, 0.),
            p(1., width, 0.),
        ])
        .unwrap();
        let prepared = PreparedSurfaceGeodesic::new(&s).unwrap();
        let start = Point2::try_new(0.5, 0.5).unwrap();
        let result = prepared
            .point(start, unit(0., 1., 0.), width / 8., tol(width))
            .unwrap();
        assert!(
            result.point.distance_to(p(1., width * 0.625, 0.)).unwrap() < tol(width).absolute()
        );
        assert!(matches!(
            prepared.point(start, unit(0., 1., 0.), width * 0.75, tol(width)),
            Err(GeometryError::ParameterCurveOutsideSurfaceDomain)
        ));
    }
}

#[test]
fn affine_charts_take_a_direct_step_but_small_warps_keep_the_ode() {
    let plane =
        NurbsSurface::try_bilinear([p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.)])
            .unwrap();
    let start = Point2::try_new(0.3, 0.4).unwrap();
    let result = PreparedSurfaceGeodesic::new(&plane)
        .unwrap()
        .point(start, unit(1., 0., 0.), 0.2, tol(1.))
        .unwrap();
    assert_eq!(result.attempted_steps, 0);
    assert!(result.point.distance_to(p(0.5, 0.4, 0.)).unwrap() < 1e-14);
    let warped =
        NurbsSurface::try_bilinear([p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 1e-5), p(0., 1., 0.)])
            .unwrap();
    let (_, du, _) = warped
        .evaluate_with_derivatives(start.x(), start.y())
        .unwrap();
    let result = PreparedSurfaceGeodesic::new(&warped)
        .unwrap()
        .point(
            start,
            du.normalized_nonzero().unwrap(),
            0.2,
            Tolerance::try_new(1e-13, 1e-14, 1e-10).unwrap(),
        )
        .unwrap();
    assert!(result.attempted_steps > 0);
    assert!((result.point.z() - 1e-5 * result.point.x() * result.point.y()).abs() < 1e-15);
}
