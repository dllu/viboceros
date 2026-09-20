use super::*;

fn point(p: [Real; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}
fn patch(corners: [[Real; 3]; 4]) -> NurbsSurface {
    NurbsSurface::try_bilinear(corners.map(point)).unwrap()
}
fn assert_closest(surface: &NurbsSurface, target: [Real; 3], expected: [Real; 3]) {
    let target = point(target);
    let parameters = surface
        .closest_affine_parameters(target)
        .expect("affine path");
    assert_eq!(
        surface
            .closest_parameters(target, Tolerance::DEFAULT)
            .unwrap(),
        parameters
    );
    let actual = surface.evaluate(parameters.0, parameters.1).unwrap();
    assert!(
        actual.distance_to(point(expected)).unwrap() < 1e-9,
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn affine_closest_checks_skew_edges_instead_of_clamping_both_coordinates() {
    let surface = patch([[0., 0., 0.], [4., 0., 0.], [6., 2., 0.], [2., 2., 0.]]);
    for (target, expected) in [
        ([2.3, 0.7, 2.], [2.3, 0.7, 0.]),
        ([0., 1., 3.], [0.5, 0.5, 0.]),
        ([3., 3., 1.], [3., 2., 0.]),
        ([7., -1., 4.], [5., 1., 0.]),
        ([1., -2., 3.], [1., 0., 0.]),
        ([-2., -3., 0.], [0., 0., 0.]),
        ([9., 6., 0.], [6., 2., 0.]),
    ] {
        assert_closest(&surface, target, expected);
    }
}

#[test]
fn affine_closest_preserves_nonunit_extreme_domains_and_distant_queries() {
    let surface = patch([[0., 0., 0.], [4., 0., 0.], [4., 2., 0.], [0., 2., 0.]]);
    for (u, v) in [
        (0.0..=1.0, 0.0..=1.0),
        (-2e12..=6e12, 1e-12..=5e-12),
        (0.0..=1e-308, 0.0..=1e308),
        (-Real::MAX..=Real::MAX, 0.0..=1.0),
    ] {
        let surface = surface.try_reparameterized(u, v).unwrap();
        assert_closest(&surface, [1.37, 0.63, 1e15], [1.37, 0.63, 0.]);
        assert_closest(&surface, [-1., 0.7, 2.], [0., 0.7, 0.]);
    }
}

#[test]
fn affine_closest_exact_guard_rejects_tiny_warps_and_rounded_diagonal_equalities() {
    for surface in [
        patch([[0., 0., 0.], [4., 0., 0.], [4., 2., 1e-18], [0., 2., 0.]]),
        patch([[0., 0., 0.], [1e16, 0., 0.], [1e16, 1., 0.], [1., 1., 0.]]),
        patch([[0., 0., 0.], [4., 0., 0.], [3., 2., 0.], [0., 2., 0.]]),
    ] {
        assert!(
            surface
                .closest_affine_parameters(point([1., 1., 1.]))
                .is_none()
        );
    }
    let surface = patch([[0., 0., 0.], [4., 0., 0.], [4., 2., 0.], [0., 2., 0.]]);
    let mut controls = surface.control_points.clone();
    for index in [1, 3] {
        controls[index] =
            WeightedPoint3::try_new(surface.control_points[index].point(), 2.).unwrap();
    }
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        controls,
        surface.knots_u.clone(),
        surface.knots_v.clone(),
    )
    .unwrap();
    assert!(
        surface
            .closest_affine_parameters(point([2., 1., 3.]))
            .is_none()
    );
    let (u, v) = surface
        .closest_parameters(point([2., 1., 3.]), Tolerance::DEFAULT)
        .unwrap();
    assert!((u - 1. / 3.).abs() < 1e-9 && (v - 0.5).abs() < 1e-9);
}

#[test]
fn affine_closest_matches_orthogonal_projection_in_rotated_translated_frames() {
    let origin = [100., -200., 300.];
    let u = [2., 1., 2.];
    let v = [2., -2., -1.];
    let n = [1., 2., -2.];
    let p = |a: Real, b: Real, h: Real| {
        std::array::from_fn(|i| origin[i] + a * u[i] + b * v[i] + h * n[i])
    };
    let surface = patch([p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.)]);
    for a in [-1., 0., 0.25, 0.75, 1., 2.] {
        for b in [-1., 0., 0.25, 0.75, 1., 2.] {
            assert_closest(
                &surface,
                p(a, b, 2.),
                p(a.clamp(0., 1.), b.clamp(0., 1.), 0.),
            );
        }
    }
}

#[test]
fn affine_closest_satisfies_independent_convex_optimality_conditions() {
    let origin = [13., -27., 3.];
    let u_axis = Vector3::try_new(4., 0., 1.).unwrap();
    for skew in -8..=8 {
        let v_axis = Vector3::try_new(Real::from(skew), 3., -1.).unwrap();
        let p = |u: Real, v: Real| {
            std::array::from_fn(|i| origin[i] + u * u_axis.to_array()[i] + v * v_axis.to_array()[i])
        };
        let surface = patch([p(0., 0.), p(1., 0.), p(1., 1.), p(0., 1.)]);
        let before = surface.clone();
        for x in [-10., -1., 0.3, 4., 10., 20.] {
            for y in [-10., 2., 7.] {
                for z in [-2., 0., 8.] {
                    let target = point([x, y, z]);
                    let (u, v) = surface.closest_affine_parameters(target).unwrap();
                    let residual = target.vector_to(surface.evaluate(u, v).unwrap()).unwrap();
                    for (parameter, axis) in [(u, u_axis), (v, v_axis)] {
                        let gradient = residual.dot(axis).unwrap();
                        let valid = if parameter <= 1e-12 {
                            gradient >= -1e-8
                        } else if parameter >= 1. - 1e-12 {
                            gradient <= 1e-8
                        } else {
                            gradient.abs() < 1e-8
                        };
                        assert!(
                            valid,
                            "skew={skew}, target={target:?}, uv=({u},{v}), gradient={gradient}"
                        );
                    }
                }
            }
        }
        assert_eq!(surface, before);
    }
}

#[test]
fn affine_closest_handles_uniform_signed_weights_and_reversed_parameter_axes() {
    let source = patch([[0., 0., 0.], [4., 0., 0.], [4., 2., 0.], [0., 2., 0.]]);
    for weight in [1., 2., -1., Real::MIN_POSITIVE, Real::MAX] {
        let surface = NurbsSurface::try_new_rational(
            1,
            1,
            2,
            2,
            source
                .control_points
                .iter()
                .map(|p| WeightedPoint3::try_new(p.point(), weight).unwrap())
                .collect(),
            source.knots_u.clone(),
            source.knots_v.clone(),
        )
        .unwrap();
        for surface in [
            surface.clone(),
            surface.try_reversed_u().unwrap(),
            surface.try_reversed_v().unwrap(),
        ] {
            assert_closest(&surface, [1.37, 0.63, 3.], [1.37, 0.63, 0.]);
        }
    }
}

#[test]
fn affine_closest_rejects_refined_and_singular_representations() {
    let source = patch([[0., 0., 0.], [4., 0., 0.], [4., 2., 0.], [0., 2., 0.]]);
    let refined = source.try_insert_knot_u(0.5, 1).unwrap();
    assert!(
        refined
            .closest_affine_parameters(point([1.37, 0.63, 3.]))
            .is_none()
    );
    let (u, v) = refined
        .closest_parameters(point([1.37, 0.63, 3.]), Tolerance::DEFAULT)
        .unwrap();
    assert!(
        refined
            .evaluate(u, v)
            .unwrap()
            .distance_to(point([1.37, 0.63, 0.]))
            .unwrap()
            < 1e-9
    );
    let singular = patch([[0., 0., 0.], [1., 0., 0.], [2., 0., 0.], [1., 0., 0.]]);
    assert!(
        singular
            .closest_affine_parameters(point([0.5, 0.5, 1.]))
            .is_none()
    );
}

#[test]
fn affine_closest_preserves_precision_on_nearly_parallel_axes() {
    let surface = patch([
        [0., 0., 0.],
        [1., 0., 0.],
        [1e15 + 1., 1., 0.],
        [1e15, 1., 0.],
    ]);
    // Native parameters are ill-conditioned here; check the evaluated model
    // point, not independent errors in the strongly coupled U/V parameters.
    assert_closest(&surface, [5e14 + 0.5, 0.5, 1.], [5e14 + 0.5, 0.5, 0.]);
}
