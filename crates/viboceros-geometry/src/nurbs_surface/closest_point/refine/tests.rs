use super::*;

fn vector(v: [Real; 3]) -> Vector3 {
    Vector3::try_from(v).unwrap()
}

fn point(p: [Real; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}

#[test]
fn curvature_step_solves_the_full_mixed_hessian_in_scaled_coordinates() {
    // S = (u, v, u² + uv + 2v²) at the origin. For residual (2,3,-1),
    // the squared-distance Hessian is [[3,1],[1,5]] and its gradient is (2,3).
    // Independent U/V reparameterization must only rescale the parameter step.
    for su in [1e-100, 1., 1e100] {
        for sv in [1e-100, 1., 1e100] {
            let jet = SurfaceJet2 {
                point: point([0.; 3]),
                derivative_u: vector([su, 0., 0.]),
                derivative_v: vector([0., sv, 0.]),
                derivative_uu: vector([0., 0., 2. * su * su]),
                derivative_uv: vector([0., 0., su * sv]),
                derivative_vv: vector([0., 0., 4. * sv * sv]),
            };
            let step = curvature_step(
                jet,
                vector([2., 3., -1.]),
                [vector([1., 0., 0.]), vector([0., 1., 0.])],
                [false; 2],
            )
            .unwrap();
            assert!((step[0] * su - 0.5).abs() < 1e-14);
            assert!((step[1] * sv - 0.5).abs() < 1e-14);
        }
    }
}

#[test]
fn curvature_step_rejects_indefinite_singular_and_unrepresentable_metrics() {
    let directions = [vector([1., 0., 0.]), vector([0., 1., 0.])];
    let jet = SurfaceJet2 {
        point: point([0.; 3]),
        derivative_u: directions[0],
        derivative_v: directions[1],
        derivative_uu: vector([0., 0., 2.]),
        derivative_uv: vector([0.; 3]),
        derivative_vv: vector([0.; 3]),
    };
    for height in [0.5, 1., Real::MAX] {
        assert!(curvature_step(jet, vector([1., 2., height]), directions, [false; 2]).is_none());
    }
    for speed in [1e-200, 1e200] {
        let jet = SurfaceJet2 {
            derivative_u: vector([speed, 0., 0.]),
            ..jet
        };
        assert!(curvature_step(jet, vector([1., 2., -1.]), directions, [false; 2]).is_none());
    }
}

#[test]
fn curvature_step_uses_the_skew_metric_and_solves_only_free_boundary_directions() {
    let jet = SurfaceJet2 {
        point: point([0.; 3]),
        derivative_u: vector([2., 0., 0.]),
        derivative_v: vector([1., 3., 0.]),
        derivative_uu: vector([0.; 3]),
        derivative_uv: vector([0.; 3]),
        derivative_vv: vector([0.; 3]),
    };
    let directions =
        [jet.derivative_u, jet.derivative_v].map(|d| d.normalized_nonzero().unwrap().as_vector());
    for (fixed, expected) in [
        ([false, false], [5. / 6., 4. / 3.]),
        ([true, false], [0., 1.5]),
        ([false, true], [1.5, 0.]),
        ([true, true], [0., 0.]),
    ] {
        let actual = curvature_step(jet, vector([3., 4., 5.]), directions, fixed).unwrap();
        for i in 0..2 {
            assert!((actual[i] - expected[i]).abs() < 1e-14);
        }
    }
}

#[test]
fn closest_point_polish_is_local_and_respects_boundary_kkt_signs() {
    let surface = paraboloid();
    let initial = (0.1, 0.2);
    assert_eq!(
        SurfaceQuery::new(&surface).polish_closest_parameters(
            point([0.3, -1.3, -0.84]),
            initial,
            Tolerance::DEFAULT
        ),
        initial
    );
    for (parameter, outward) in [(0., -1.), (1., 1.)] {
        for gradient in [-1., 0., 1.] {
            let active = active_constraints([parameter, 0.5], [[0., 1.]; 2], [gradient, 2.]);
            assert_eq!(active, [gradient * outward >= 0., false]);
            let expected = if active[0] { 2. } else { gradient.hypot(2.) };
            assert_eq!(projected_gradient_norm([gradient, 2.], active), expected);
        }
    }
}

fn paraboloid() -> NurbsSurface {
    let coordinates = [-1., 0., 1.];
    let square = [1., -1., 1.];
    let controls = (0..3)
        .flat_map(|j| {
            (0..3).map(move |i| {
                point([
                    coordinates[i],
                    coordinates[j],
                    square[i] + coordinates[i] * coordinates[j] + 2. * square[j],
                ])
            })
        })
        .collect();
    NurbsSurface::try_clamped_uniform(2, 2, 3, 3, controls).unwrap()
}

fn assert_projection(surface: &NurbsSurface, target: Point3, expected: Point3) {
    let (u, v) = surface
        .closest_parameters(target, Tolerance::DEFAULT)
        .unwrap();
    let actual = surface.evaluate(u, v).unwrap();
    assert!(
        actual.distance_to(expected).unwrap() < 1e-8,
        "target={target:?}, actual={actual:?}, expected={expected:?}"
    );
}

#[test]
fn closest_point_curvature_handles_mixed_partials_edges_and_corners() {
    let source = paraboloid();
    let before = source.clone();
    for (domain_u, domain_v) in [(0.0..=1.0, 0.0..=1.0), (-2e12..=6e12, 1e-12..=5e-12)] {
        let surface = source.try_reparameterized(domain_u, domain_v).unwrap();
        for x in [-1., -0.65, -0.2, 0.31, 0.7, 1.] {
            for y in [-1., -0.65, -0.2, 0.31, 0.7, 1.] {
                let z = x * x + x * y + 2. * y * y;
                let expected = point([x, y, z]);
                // The target is below the entire convex graph. Its squared
                // distance objective is strictly convex. The residual gives
                // zero interior gradient or the proper outward boundary sign,
                // proving that this prescribed point is its global minimum.
                let outward = |t: Real| if t.abs() == 1. { 3. * t } else { 0. };
                let target = point([
                    x + 8. * (2. * x + y) + outward(x),
                    y + 8. * (x + 4. * y) + outward(y),
                    z - 8.,
                ]);
                assert_projection(&surface, target, expected);
            }
        }
    }
    assert_eq!(source, before);
}

#[test]
fn closest_point_curvature_falls_back_when_only_second_derivatives_overflow() {
    let source = paraboloid();
    let surface = source
        .try_reparameterized(0.0..=1e-170, 0.0..=1e160)
        .unwrap();
    let (u, v) = (
        surface.parameter_at_u(0.6).unwrap(),
        surface.parameter_at_v(0.35).unwrap(),
    );
    assert!(surface.evaluate_with_second_derivatives(u, v).is_err());
    assert!(surface.evaluate_with_derivatives(u, v).is_ok());
    // (x,y,z) = (0.2,-0.3,0.16), normal = (-0.1,1,1).
    assert_projection(
        &surface,
        point([0.3, -1.3, -0.84]),
        point([0.2, -0.3, 0.16]),
    );
}

#[test]
fn closest_point_curvature_retains_multistart_search_past_a_saddle() {
    let surface = NurbsSurface::try_bilinear([
        point([-1., -1., 2.]),
        point([1., -1., -2.]),
        point([1., 1., 2.]),
        point([-1., 1., -2.]),
    ])
    .unwrap();
    let target = point([0., 0., 1.]);
    let (u, v) = surface
        .closest_parameters(target, Tolerance::DEFAULT)
        .unwrap();
    let actual = surface.evaluate(u, v).unwrap();
    // The origin is a stationary saddle. The two true minima are
    // (+/-0.5,+/-0.5,0.5), with equal signs and squared distance 3/4.
    assert!((actual.x().abs() - 0.5).abs() < 1e-8);
    assert!((actual.y() - actual.x()).abs() < 1e-8);
    assert!((actual.z() - 0.5).abs() < 1e-8);
    assert!((actual.distance_to(target).unwrap() - 0.75_f64.sqrt()).abs() < 1e-12);
}
